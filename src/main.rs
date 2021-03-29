#![allow(clippy::single_component_path_imports)]

use std::{
    collections::{hash_map::Entry, HashMap},
    convert::{Infallible, TryFrom, TryInto},
    error::Error,
    ffi::OsString,
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

use async_std::{
    path::PathBuf as AsyncPathBuf,
    sync::{Arc, Mutex},
    task,
};

use bincode;

use hyper::StatusCode;

use notify::{
    event::Event as NotifyEvent, RecommendedWatcher, RecursiveMode,
    Result as NotifyResult, Watcher,
};

use url::{self, Url};

use warp::{self, Filter, Rejection};
use warp_sessions::{
    self, CookieOptions, MemoryStore, SameSiteCookieOption, SessionWithStore,
};

use webbrowser;

use gmtool_common::{FileEntry, PortableOsString};

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
        path: OsString,
    },
}
async fn router_handler(
    mut message_rx: Receiver<RouterMessage>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connections: HashMap<String, Sender<_>> = HashMap::new();
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
        eprintln!("Event msg {:?}", &msg);
        match msg {
            RouterMessage::NewConnection {
                id,
                event_tx,
                shutdown_rx,
            } => {
                // TODO: Reply with refusal if session SSE already open
                if let Entry::Vacant(entry) = connections.entry(id.clone()) {
                    entry.insert(event_tx);
                }

                seen_any_sse = true;
                shutdown_futures.push(shutdown_rx.into_future());
            }
            RouterMessage::FileChange { path } => {
                let path: PortableOsString = path.into();
                for event_tx in connections.values() {
                    // TODO: Find a way for stream shutdown to return event_rx
                    if let Err(e) = event_tx.unbounded_send(
                        warp::filters::sse::Event::default()
                            .event(String::from("file_change"))
                            .json_data(&path),
                    ) {
                        eprintln!("Closed connection exists in map: {:?}", e);
                    }
                }
            }
        }
    }
    drop(connections);
    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (message_tx, message_rx) = unbounded_channel::<RouterMessage>();
    let router = router_handler(message_rx);

    // Spawn file watcher thread
    let watcher_message_tx = message_tx.clone();
    let watcher: RecommendedWatcher =
        Watcher::new_immediate(move |res: NotifyResult<NotifyEvent>| {
            // TODO: How can we do error handling
            // when this callback doesn't have a Result return type?
            let event = res.unwrap();
            for path in event.paths {
                eprintln!(
                    "Got notify event on path: {}",
                    path.to_str().unwrap()
                );
                watcher_message_tx
                    .unbounded_send(RouterMessage::FileChange {
                        path: path.into(),
                    })
                    .unwrap();
            }
        })?;
    let watcher_ref = Arc::new(Mutex::new(watcher));

    let session_store = MemoryStore::new();
    let cookie_options = Some(CookieOptions {
        cookie_name: "sid",
        cookie_value: None,
        max_age: Some(60),
        domain: None,
        path: None,
        secure: false,
        http_only: true,
        same_site: Some(SameSiteCookieOption::Strict),
    });
    let with_session =
        warp_sessions::request::with_session(session_store, cookie_options);

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
    let post_chdir = warp::path("chdir")
        .and(warp::post())
        .and(with_session.clone())
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let path: PortableOsString = match bincode::deserialize(&body) {
                    Ok(path) => path,
                    Err(e) => {
                        let msg = format!("Path deserialize failed: {:?}", e);
                        eprintln!("{}", msg);
                        return Ok::<_, Rejection>((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::BAD_REQUEST,
                            ),
                            session_with_store,
                        ));
                    }
                };
                let path: PathBuf = path.try_into().expect("native OsString");

                let session = &mut session_with_store.session;
                let mut curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();
                // Initialize default current dir
                if curdir.is_none() {
                    match std::env::current_dir() {
                        Ok(dir) => curdir = Some(dir),
                        Err(e) => eprintln!("Current dir missing {:?}", e),
                    }
                }

                // Update current directory
                let chdir_result = if let Some(ref mut curdir) = curdir {
                    curdir.push(path);
                    match tokio::fs::canonicalize(&curdir).await {
                        Ok(path) => *curdir = path,
                        Err(e) => {
                            // Warn that canonicalize failed,
                            // but keep using curdir
                            eprintln!(
                                "Canonicalize for {:?} failed: {:?}",
                                curdir, e
                            );
                        }
                    }
                    match bincode::serialize(&PortableOsString::from(
                        curdir.clone(),
                    )) {
                        Ok(bytes) => {
                            warp::reply::with_status(bytes, StatusCode::OK)
                        }
                        Err(e) => {
                            let msg =
                                format!("Couldn't serialize result: {:?}", e);
                            eprintln!("{}", msg);
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::PRECONDITION_REQUIRED,
                            )
                        }
                    }
                } else if path.is_absolute() {
                    curdir = Some(path.clone());
                    match bincode::serialize(&PortableOsString::from(path)) {
                        Ok(bytes) => {
                            warp::reply::with_status(bytes, StatusCode::OK)
                        }
                        Err(e) => {
                            let msg =
                                format!("Couldn't serialize result: {:?}", e);
                            eprintln!("{}", msg);
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::PRECONDITION_REQUIRED,
                            )
                        }
                    }
                } else {
                    let msg = format!(
                        "Couldn't chdir to {:?}, \
                         path is relative and have no \
                         current directory",
                        path
                    );
                    eprintln!("{}", msg);
                    warp::reply::with_status(
                        msg.into_bytes(),
                        StatusCode::PRECONDITION_REQUIRED,
                    )
                };

                match session.insert("curdir", curdir) {
                    Ok(()) => (),
                    Err(e) => {
                        let msg = format!("Couldn't serialize path: {:?}", e);
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::PRECONDITION_REQUIRED,
                            ),
                            session_with_store,
                        ));
                    }
                }

                Ok((chdir_result, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session);
    let post_lsdir = warp::path("lsdir")
        .and(warp::post())
        .and(with_session.clone())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>| async move {
                let session = &mut session_with_store.session;
                let curdir = session
                    .get::<Option<PathBuf>>("curdir")
                    .unwrap_or_default();

                let path = if let Some(path) = curdir {
                    path
                } else {
                    let msg = "No current directory to list".to_string();
                    eprintln!("{}", msg);
                    return Ok::<_, Rejection>((
                        warp::reply::with_status(
                            msg.into_bytes(),
                            StatusCode::PRECONDITION_REQUIRED,
                        ),
                        session_with_store,
                    ))
                };

                let list = match get_dir_file_entries(&path.into()).await {
                    Ok(list) => list,
                    Err(e) => {
                        let msg =
                            format!("Couldn't list dir: {:?}", e);
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::INTERNAL_SERVER_ERROR,
                            ),
                            session_with_store,
                        ))
                    }
                };

                let reply = match bincode::serialize(&list) {
                    Ok(bytes) => warp::reply::with_status(bytes, StatusCode::OK),
                    Err(e) => {
                       let msg =
                           format!("Couldn't serialize result: {:?}", e);
                       eprintln!("{}", msg);
                       warp::reply::with_status(
                           msg.into_bytes(),
                           StatusCode::PRECONDITION_REQUIRED,
                       )
                    }
                };
                Ok((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session);
    let post_read = warp::path("read")
        .and(warp::post())
        .and(with_session.clone())
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| async move {
                let path: PortableOsString = match bincode::deserialize(&body) {
                    Ok(path) => path,
                    Err(e) => {
                        let msg = format!("Path deserialize failed: {:?}", e);
                        eprintln!("{}", msg);
                        return Ok::<_, Rejection>((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::BAD_REQUEST,
                            ),
                            session_with_store,
                        ));
                    }
                };
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
                    let msg = format!(
                        "Couldn't read {:?}, \
                         path is relative and have no \
                         current directory",
                        path
                    );
                    eprintln!("{}", msg);
                    return Ok((
                        warp::reply::with_status(
                            msg.into_bytes(),
                            StatusCode::PRECONDITION_REQUIRED,
                        ),
                        session_with_store,
                    ));
                };

                let s = match tokio::fs::read_to_string(&path).await {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("Couldn't read {:?}: {:?}", path, e);
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::NOT_FOUND,
                            ),
                            session_with_store,
                        ));
                    }
                };
                let file: gcs::FileKind = match serde_json::from_str(s.as_str())
                {
                    Ok(file) => file,
                    Err(e) => {
                        let msg = format!("Couldn't read {:?}: {:?}", path, e);
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::NOT_FOUND,
                            ),
                            session_with_store,
                        ));
                    }
                };

                let reply = match serde_cbor::to_vec(&file) {
                    Ok(vec) => warp::reply::with_status(vec, StatusCode::OK),
                    Err(e) => {
                        let msg = format!("Serialize failed: {:?}", e);
                        eprintln!("{}", msg);
                        warp::reply::with_status(
                            msg.into_bytes(),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    }
                };
                Ok((reply, session_with_store))
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session);
    let get_sse = warp::path("sse")
        .and(warp::get())
        .and(with_session.clone())
        .and_then(move |session_with_store: SessionWithStore<MemoryStore>| {
            let message_tx = message_tx.clone();
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
        });
    let post_watch = warp::path("watch")
        .and(warp::post())
        .and(with_session.clone())
        .and(warp::body::bytes())
        .and_then(
            move |mut session_with_store: SessionWithStore<MemoryStore>,
                  body: hyper::body::Bytes| {
                let watcher_ref = Arc::clone(&watcher_ref);
                async move {
                    let path: PortableOsString =
                        match bincode::deserialize(&body) {
                            Ok(path) => path,
                            Err(e) => {
                                let msg =
                                    format!("Path deserialize failed: {:?}", e);
                                eprintln!("{}", msg);
                                return Ok::<_, Rejection>((
                                    warp::reply::with_status(
                                        msg.into_bytes(),
                                        StatusCode::BAD_REQUEST,
                                    ),
                                    session_with_store,
                                ));
                            }
                        };
                    let path: PathBuf =
                        path.try_into().expect("native OsString");

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
                        let msg = format!(
                            "Couldn't watch {:?}, \
                             path is relative and have no \
                             current directory",
                            path
                        );
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::PRECONDITION_REQUIRED,
                            ),
                            session_with_store,
                        ));
                    };

                    if let Err(e) = watcher_ref.lock_arc().await.watch(
                        OsString::try_from(path.clone())
                            .expect("Native os string"),
                        RecursiveMode::NonRecursive,
                    ) {
                        let msg = format!("Watch {:?} failed {:?}", path, e);
                        eprintln!("{}", msg);
                        return Ok((
                            warp::reply::with_status(
                                msg.into_bytes(),
                                StatusCode::INTERNAL_SERVER_ERROR,
                            ),
                            session_with_store,
                        ));
                    }
                    Ok((
                        warp::reply::with_status(vec![], StatusCode::OK),
                        session_with_store,
                    ))
                }
            },
        )
        .untuple_one()
        .and_then(warp_sessions::reply::with_session);

    let routes = get_webui
        .or(post_chdir)
        .or(post_lsdir)
        .or(post_read)
        .or(get_sse)
        .or(post_watch);
    let http_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let (http_addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(http_addr, async {
            router.await.ok();
        });

    println!("Bound server to {:?}", http_addr);

    let url = format!("http://{}", http_addr.to_string());
    let mut url =
        Url::parse_with_params(&url, &[("agentaddr", url.to_string())])?;
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
