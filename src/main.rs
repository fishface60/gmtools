#![allow(clippy::single_component_path_imports, clippy::upper_case_acronyms)]

use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    convert::{Infallible, TryInto},
    error::Error,
    io::Error as IOError,
    net::SocketAddr,
    path::PathBuf,
};

use futures::{
    channel::mpsc::{
        unbounded as unbounded_channel, UnboundedReceiver as Receiver,
        UnboundedSender as Sender,
    },
    prelude::*,
    select,
    stream::StreamExt,
};
use pin_project::pin_project;

use async_std::{path::PathBuf as AsyncPathBuf, task};

use bincode;

use hyper::StatusCode;

use rand::Rng;

use notify::{
    event::{
        CreateKind, Event as NotifyEvent, EventKind, ModifyKind, RenameMode,
    },
    RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher,
};

use url::{self, Url};

use warp::{self, reject::Reject, Filter, Rejection, Reply};
use warp_sessions::{
    self, CookieOptions, MemoryStore, SameSiteCookieOption, SessionWithStore,
};

use webbrowser;

use gmtool_common::{FileEntry, PortableOsString, ReadResponse, WriteRequest};

const WEBUI: &str = include_str!(env!("WEBUI_HTML_PATH"));

async fn get_dir_file_entries(
    path: &AsyncPathBuf,
) -> Result<Vec<FileEntry>, IOError> {
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut dents = path.read_dir().await?;
    while let Some(dirent) = dents.next().await {
        let dirent = dirent?;
        let metadata = dirent.metadata().await?;
        let name = dirent.file_name();
        if metadata.is_dir() {
            entries.push(FileEntry::Directory(name.into()));
            continue;
        }
        if metadata.is_file() {
            let path: PathBuf = name.into();
            let is_gcs_file = match path.extension() {
                None => false,
                Some(os_str) => os_str == "gcs",
            };
            if is_gcs_file {
                entries.push(FileEntry::GCSFile(path.into()));
            }
        }
    }
    Ok(entries)
}

async fn launch_browser(url: Url) -> Result<(), std::io::Error> {
    webbrowser::open(&url.as_str())?;
    Ok(())
}

#[derive(Debug)]
enum Void {}

// From SSE handler or Notify to Router
#[derive(Debug)]
enum RouterMessage {
    NewConnection {
        id: String,
        event_tx: Sender<Result<warp::sse::Event, serde_json::Error>>,
        shutdown_rx: Receiver<Void>,
    },
    FileChange {
        event: NotifyEvent,
    },
    AddWatch {
        id: String,
        path: PathBuf,
    },
}
async fn router_handler(
    mut message_rx: Receiver<RouterMessage>,
    mut watcher: RecommendedWatcher,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connections: HashMap<String, Sender<_>> = HashMap::new();
    let mut watches: HashMap<String, HashSet<PathBuf>> = HashMap::new();
    let mut shutdown_futures = stream::FuturesUnordered::new();
    let mut seen_any_sse = false;
    loop {
        let msg = select! {
            msg = message_rx.next().fuse() => match msg {
                None => break,
                Some(msg) => msg,
            },
            res = shutdown_futures.next() => {
                match res {
                    // NOTE: We get a None every time the shutdown queue empties
                    //       If we haven't yet encountered any shutdowns then
                    //       we continue, expecting a message to add shutdown.
                    None => if seen_any_sse {
                        break;
                    }
                    Some((shutdown_res, _shutdown_rx)) => {
                        if let Some(void) = shutdown_res {
                            match void {}
                        }
                    }
                }
                continue;
            }
        };
        log::debug!("Event msg {:?}", &msg);
        match msg {
            RouterMessage::NewConnection {
                id,
                event_tx,
                shutdown_rx,
            } => {
                // TODO: Reply with refusal if session SSE already open
                if let Entry::Vacant(entry) = connections.entry(id) {
                    entry.insert(event_tx);
                }

                seen_any_sse = true;
                shutdown_futures.push(shutdown_rx.into_future());
            }
            RouterMessage::FileChange { event } => {
                // If it's a Modify(Name(To)) event request a watch on the file
                // if there's any connections interested in it.
                // Only send path changes to connections which care.
                let path = &event.paths[0];
                let any_watching =
                    watches.values().flatten().any(|p| p == path);
                let is_rename = matches!(
                    event.kind,
                    EventKind::Modify(ModifyKind::Name(RenameMode::To))
                );
                if any_watching && is_rename {
                    watcher.watch(path, RecursiveMode::NonRecursive)?;
                }
                for (id, event_tx) in &connections {
                    match watches.get(id) {
                        Some(paths) => {
                            if !paths.contains(path) {
                                continue;
                            }
                        }
                        None => continue,
                    }
                    let path: PortableOsString = path.clone().into();
                    // TODO: Find a way for stream shutdown to return event_rx
                    if let Err(e) = event_tx.unbounded_send(
                        warp::filters::sse::Event::default()
                            .event(String::from("file_change"))
                            .json_data(&path),
                    ) {
                        log::error!("Closed connection exists in map: {:?}", e);
                    }
                }
            }
            RouterMessage::AddWatch { id, path } => {
                // TODO: What to do with error?
                watcher.watch(
                    path.parent().expect("has parent"),
                    RecursiveMode::NonRecursive,
                )?;
                watcher.watch(&path, RecursiveMode::NonRecursive)?;
                if let Entry::Vacant(entry) = watches.entry(id.clone()) {
                    entry.insert(HashSet::new());
                }
                let paths = watches.get_mut(&id).expect("watch entry inserted");
                paths.insert(path);
            }
        }
    }
    drop(connections);
    Ok(())
}

#[derive(Debug)]
enum Rejections {
    MalformedBody(Box<bincode::ErrorKind>),
    SessionDataUnserializable(serde_json::Error),
    BincodeReplyUnserializable(Box<bincode::ErrorKind>),
    NoCurdirRelativePath(PathBuf),
    NoCurdirToLs,
    LsDirError(std::io::Error),
    ReadError(PathBuf, std::io::Error),
    SheetParseError(PathBuf, serde_json::Error),
    SheetSerializeError(serde_json::Error),
    WriteError(PathBuf, std::io::Error),
    Unauthorized,
}

impl Reject for Rejections {}

async fn authorize(
    session_with_store: SessionWithStore<MemoryStore>,
) -> Result<SessionWithStore<MemoryStore>, Rejection> {
    let session = &session_with_store.session;
    let authenticated: bool = session.get("authenticated").unwrap_or_default();
    if !authenticated {
        return Err(warp::reject::custom(Rejections::Unauthorized));
    }
    Ok(session_with_store)
}

async fn handle_rejection(
    err: Rejection,
) -> std::result::Result<impl Reply, Rejection> {
    let (code, message) = if let Some(e) = err.find::<Rejections>() {
        match e {
            Rejections::MalformedBody(e) => {
                let msg = format!("Deserialize failed: {:?}", e);
                log::error!("{}", msg);
                (StatusCode::BAD_REQUEST, msg)
            }
            Rejections::SessionDataUnserializable(e) => {
                let msg = format!("Session data serialize failed: {:?}", e);
                log::error!("{}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            Rejections::BincodeReplyUnserializable(e) => {
                let msg = format!("Reply serialize failed: {:?}", e);
                log::error!("{}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            Rejections::NoCurdirRelativePath(path) => {
                let msg = format!(
                    "Couldn't use path {:?}, path is relative and have no \
                     current directory",
                    path
                );
                log::error!("{}", msg);
                (StatusCode::PRECONDITION_REQUIRED, msg)
            }
            Rejections::NoCurdirToLs => {
                let msg = "No current directory to list".to_string();
                log::error!("{}", msg);
                (StatusCode::PRECONDITION_REQUIRED, msg)
            }
            Rejections::LsDirError(e) => {
                let msg = format!("lsdir failed: {:?}", e);
                log::error!("{}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            Rejections::ReadError(path, e) => {
                let msg = format!("Couldn't read {:?}: {:?}", path, e);
                log::error!("{}", msg);
                (StatusCode::NOT_FOUND, msg)
            }
            Rejections::SheetParseError(path, e) => {
                let msg = format!("Couldn't parse {:?}: {:?}", path, e);
                log::error!("{}", msg);
                (StatusCode::NOT_FOUND, msg)
            }
            Rejections::SheetSerializeError(e) => {
                let msg = format!("Couldn't serialise sheet: {:?}", e);
                log::error!("{}", msg);
                (StatusCode::BAD_REQUEST, msg)
            }
            Rejections::WriteError(path, e) => {
                let msg = format!("Couldn't write {:?}: {:?}", path, e);
                log::error!("{}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            Rejections::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "Unauthorized".to_string())
            }
        }
    } else {
        // propagate err
        return Err(err);
    };

    Ok(warp::reply::with_status(message, code))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    pretty_env_logger::init();
    let (message_tx, message_rx) = unbounded_channel::<RouterMessage>();

    // Spawn file watcher thread
    let watcher_message_tx = message_tx.clone();
    let watcher: RecommendedWatcher =
        Watcher::new_immediate(move |res: NotifyResult<NotifyEvent>| {
            // TODO: How can we do error handling
            // when this callback doesn't have a Result return type?
            log::debug!("Event: {:?}", &res);
            let event = res.unwrap();
            let send_event = matches!(
                event.kind,
                EventKind::Modify(ModifyKind::Name(RenameMode::To))
                    | EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Create(CreateKind::File)
            );
            if send_event {
                watcher_message_tx
                    .unbounded_send(RouterMessage::FileChange { event })
                    .unwrap();
            }
        })?;

    let router = router_handler(message_rx, watcher);

    let session_store = MemoryStore::new();
    let cookie_options = Some(CookieOptions {
        cookie_name: "sid",
        cookie_value: None,
        max_age: None,
        domain: None,
        path: None,
        secure: false,
        http_only: true,
        same_site: Some(SameSiteCookieOption::Strict),
    });
    let with_session =
        warp_sessions::request::with_session(session_store, cookie_options);

    let mut rng = rand::thread_rng();
    let secret: u32 = rng.gen();
    let token = base64::encode(secret.to_le_bytes());

    let get_webui = warp::path("webui")
        .and(warp::get())
        .and(with_session.clone())
        .and_then(
            |session_with_store: SessionWithStore<MemoryStore>| async move {
                Ok::<_, Rejection>((
                    warp::reply::html(WEBUI),
                    session_with_store,
                ))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session);
    let post_auth = warp::path("auth")
        .and(warp::post())
        .and(with_session.clone())
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let req_secret: u32 =
                    bincode::deserialize(&body).map_err(|e| {
                        warp::reject::custom(Rejections::MalformedBody(e))
                    })?;

                let session = &mut session_with_store.session;
                if secret == req_secret {
                    session.insert("authenticated", true).map_err(|e| {
                        Rejections::SessionDataUnserializable(e)
                    })?;
                }

                Ok::<_, Rejection>((StatusCode::OK, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);
    let sse_message_tx = message_tx.clone();
    let get_sse = warp::path("sse")
        .and(warp::get())
        .and(with_session.clone())
        .and_then(authorize)
        .and_then(move |session_with_store: SessionWithStore<MemoryStore>| {
            let message_tx = sse_message_tx.clone();
            async move {
                let (event_tx, event_rx) = unbounded_channel();
                let (shutdown_tx, shutdown_rx) = unbounded_channel();
                let id = session_with_store.session.id().to_string();
                message_tx
                    .unbounded_send(RouterMessage::NewConnection {
                        id,
                        event_tx,
                        shutdown_rx,
                    })
                    .expect("Message channel live");

                #[pin_project]
                struct PayloadStream<S, P>
                where
                    S: Stream<
                        Item = Result<warp::sse::Event, serde_json::Error>,
                    >,
                {
                    #[pin]
                    inner: S,
                    payload: P,
                }
                impl<S, P> Stream for PayloadStream<S, P>
                where
                    S: Stream<
                        Item = Result<warp::sse::Event, serde_json::Error>,
                    >,
                {
                    type Item = Result<warp::sse::Event, serde_json::Error>;
                    fn poll_next(
                        self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                    ) -> core::task::Poll<Option<Self::Item>>
                    {
                        let pin = self.project();
                        S::poll_next(pin.inner, cx)
                    }
                }
                Ok::<_, Infallible>(warp::sse::reply(
                    warp::sse::keep_alive().stream(PayloadStream {
                        inner: event_rx,
                        payload: shutdown_tx,
                    }),
                ))
            }
        })
        .recover(handle_rejection);
    let post_chdir = warp::path("chdir")
        .and(warp::post())
        .and(with_session.clone())
        .and_then(authorize)
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let path: PortableOsString = bincode::deserialize(&body)
                    .map_err(|e| {
                        warp::reject::custom(Rejections::MalformedBody(e))
                    })?;
                let path: PathBuf = path.try_into().expect("native OsString");

                let session = &mut session_with_store.session;
                let mut curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();
                // Initialize default current dir
                if curdir.is_none() {
                    match std::env::current_dir() {
                        Ok(dir) => curdir = Some(dir),
                        Err(e) => log::warn!("Current dir missing {:?}", e),
                    }
                }

                // Update current directory
                let (curdir, res_path) = if let Some(mut curdir) = curdir {
                    curdir.push(path);
                    let curdir = match tokio::fs::canonicalize(&curdir).await {
                        Ok(canon) => canon,
                        Err(e) => {
                            // Warn that canonicalize failed,
                            // but keep using curdir
                            log::warn!(
                                "Canonicalize for {:?} failed: {:?}",
                                curdir,
                                e
                            );
                            curdir
                        }
                    };
                    (Some(curdir.clone()), PortableOsString::from(curdir))
                } else if path.is_absolute() {
                    curdir = Some(path.clone());
                    (curdir, PortableOsString::from(path))
                } else {
                    return Err(Rejection::from(
                        Rejections::NoCurdirRelativePath(path),
                    ));
                };
                let bytes = bincode::serialize(&res_path).map_err(|e| {
                    warp::reject::custom(
                        Rejections::BincodeReplyUnserializable(e),
                    )
                })?;
                let reply = warp::reply::with_status(bytes, StatusCode::OK);

                session
                    .insert("curdir", curdir)
                    .map_err(Rejections::SessionDataUnserializable)?;

                Ok::<_, Rejection>((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);
    let get_lsdir = warp::path("lsdir")
        .and(warp::get())
        .and(with_session.clone())
        .and_then(authorize)
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>| async move {
                let session = &mut session_with_store.session;
                let curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();

                let path = curdir.ok_or_else(||
                    warp::reject::custom(Rejections::NoCurdirToLs))?;

                let list = get_dir_file_entries(&path.into()).await.map_err(
                    Rejections::LsDirError)?;

                let bytes = bincode::serialize(&list).map_err(|e| {
                    warp::reject::custom(
                        Rejections::BincodeReplyUnserializable(e),
                    )
                })?;
                let reply = warp::reply::with_status(bytes, StatusCode::OK);
                Ok::<_, Rejection>((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);
    let post_read = warp::path("read")
        .and(warp::post())
        .and(with_session.clone())
        .and_then(authorize)
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let path: PortableOsString = bincode::deserialize(&body)
                    .map_err(|e| {
                        warp::reject::custom(Rejections::MalformedBody(e))
                    })?;
                let path: PathBuf = path.try_into().expect("native OsString");

                let session = &mut session_with_store.session;
                let curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();

                let path = if let Some(ref curdir) = curdir {
                    let mut dir = curdir.clone();
                    dir.push(path);
                    dir
                } else if path.is_absolute() {
                    path
                } else {
                    return Err(Rejection::from(
                        Rejections::NoCurdirRelativePath(path),
                    ));
                };

                let s =
                    tokio::fs::read_to_string(&path).await.map_err(|e| {
                        warp::reject::custom(Rejections::ReadError(
                            path.clone(),
                            e,
                        ))
                    })?;
                let contents: gcs::FileKind = serde_json::from_str(s.as_str())
                    .map_err(|e| {
                        warp::reject::custom(Rejections::SheetParseError(
                            path.clone(),
                            e,
                        ))
                    })?;

                let bytes = bincode::serialize(&ReadResponse {
                    path: path.into(),
                    contents,
                })
                .map_err(|e| {
                    warp::reject::custom(
                        Rejections::BincodeReplyUnserializable(e),
                    )
                })?;
                let reply = warp::reply::with_status(bytes, StatusCode::OK);
                Ok::<_, Rejection>((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);
    let watch_message_tx = message_tx;
    let post_watch = warp::path("watch")
        .and(warp::post())
        .and(with_session.clone())
        .and_then(authorize)
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| {
                let message_tx = watch_message_tx.clone();
                async move {
                    let path: PortableOsString = bincode::deserialize(&body)
                        .map_err(|e| {
                            warp::reject::custom(Rejections::MalformedBody(e))
                        })?;
                    let path: PathBuf =
                        path.try_into().expect("native OsString");

                    let session = &mut session_with_store.session;
                    let curdir = session
                        .get::<Option<PathBuf>>("curdir")
                        .unwrap_or_default();
                    let id = session.id().to_string();

                    let path = if let Some(ref curdir) = curdir {
                        let mut dir = curdir.clone();
                        dir.push(path);
                        dir
                    } else if path.is_absolute() {
                        path
                    } else {
                        return Err(Rejection::from(
                            Rejections::NoCurdirRelativePath(path),
                        ));
                    };

                    message_tx
                        .unbounded_send(RouterMessage::AddWatch { id, path })
                        .unwrap();
                    Ok::<_, Rejection>((
                        warp::reply::with_status(vec![], StatusCode::OK),
                        session_with_store,
                    ))
                }
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);
    let post_write = warp::path("write")
        .and(warp::post())
        .and(with_session.clone())
        .and_then(authorize)
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let write_req: WriteRequest = bincode::deserialize(&body)
                    .map_err(|e| {
                        warp::reject::custom(Rejections::MalformedBody(e))
                    })?;
                let path: PathBuf =
                    write_req.path.try_into().expect("native OsString");

                let session = &mut session_with_store.session;
                let curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();

                let path = if let Some(ref curdir) = curdir {
                    let mut dir = curdir.clone();
                    dir.push(path);
                    dir
                } else if path.is_absolute() {
                    path
                } else {
                    return Err(Rejection::from(
                        Rejections::NoCurdirRelativePath(path),
                    ));
                };

                let contents =
                    gcs::to_json(&write_req.contents).map_err(|e| {
                        warp::reject::custom(Rejections::SheetSerializeError(e))
                    })?;
                tokio::fs::write(&path, &contents).await.map_err(|e| {
                    warp::reject::custom(Rejections::WriteError(
                        path.clone(),
                        e,
                    ))
                })?;

                let bytes = bincode::serialize(&PortableOsString::from(path))
                    .map_err(|e| {
                    warp::reject::custom(
                        Rejections::BincodeReplyUnserializable(e),
                    )
                })?;
                let reply = warp::reply::with_status(bytes, StatusCode::OK);
                Ok::<_, Rejection>((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session)
        .recover(handle_rejection);

    let routes = get_webui
        .or(post_auth)
        .or(get_sse)
        .or(post_chdir)
        .or(get_lsdir)
        .or(post_read)
        .or(post_watch)
        .or(post_write);

    let http_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let (http_addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(http_addr, async {
            router.await.ok();
        });

    println!("Bound server to {:?}", http_addr);

    let url = format!("http://{}", http_addr.to_string());
    let mut url = Url::parse_with_params(
        &url,
        &[("agentaddr", url.to_string()), ("token", token)],
    )?;
    url.set_path("/webui");

    // NOTE: Necessarily uses an executor specific spawn
    //       Because a blocking API needs to be used.
    //       Spawning an OS thread an asynchronously sending via unbounded
    //       is not worth the effort to be executor independant
    //       and launch_browser(urls).boxed() is a task that blocks.
    // TODO: Move to tokio spawn
    let launch = task::spawn(launch_browser(url));

    let mut launch_fused = launch.fuse();
    let mut server_fused = Box::pin(server).fuse();
    loop {
        select! {
            launched = launch_fused => {
                // Launching can fail, and if it does we can't recover
                launched?;
            },
            () = server_fused => {
                // TODO: evaluate this
                //res?;
                break;
            },
        }
    }
    Ok(())
}
