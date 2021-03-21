#![allow(clippy::single_component_path_imports)]

use std::{
    collections::{hash_map::Entry, HashMap},
    convert::{TryFrom, TryInto},
    error::Error,
    ffi::OsString,
    io::Error as IOError,
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

use async_std::{
    fs,
    net::SocketAddr,
    path::PathBuf as AsyncPathBuf,
    sync::{Arc, Mutex},
    task,
};

use bincode;

use http_types::mime;

use notify::{
    event::Event as NotifyEvent, RecommendedWatcher, RecursiveMode,
    Result as NotifyResult, Watcher,
};

use tide::{
    self, listener::ListenInfo, prelude::*, sse::Sender as SSESender, Response,
};

use url::{self, Url};

use webbrowser;

use gmtool_common::{FileEntry, PortableOsString};

async fn get_webui(_req: tide::Request<()>) -> tide::Result {
    Ok(Response::builder(200)
        .body(include_str!(env!("WEBUI_HTML_PATH")))
        .content_type(mime::HTML)
        .build())
}

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

async fn launch_browser(urls: Vec<Url>) -> Result<(), std::io::Error> {
    for url in urls {
        webbrowser::open(&url.as_str())?;
    }
    Ok(())
}

#[derive(Debug)]
enum Void {}

// From SSE handler or Notify to Router
#[derive(Debug)]
enum RouterMessage {
    NewConnection {
        id: String,
        path_tx: Sender<OsString>,
        resp_tx: Sender<Sender<(String, Receiver<OsString>)>>,
        shutdown_rx: Receiver<Void>,
    },
    FileChange {
        path: OsString,
    },
}
async fn router_handler(
    mut message_rx: Receiver<RouterMessage>,
) -> Result<(), Box<dyn Error>> {
    let (disconnect_tx, mut disconnect_rx) = unbounded_channel();
    let mut connections: HashMap<String, Sender<OsString>> = HashMap::new();
    let mut shutdown_futures = stream::FuturesUnordered::new();
    let mut seen_any_sse = false;
    loop {
        let msg = select! {
            msg = message_rx.next().fuse() => match msg {
                None => break,
                Some(msg) => msg,
            },
            disconnect = disconnect_rx.next().fuse() => {
                let (id, _path_rx) = disconnect.unwrap();
                assert!(connections.remove(&id).is_some());
                continue;
            },
            res = shutdown_futures.next() => {
                eprintln!("sse shutdown {:?}", &res);
                match res {
                    // NOTE: We get a None every time the shutdown queue empties
                    //       If we haven't yet encountered any shutdowns then
                    //       we continue, expecting a message to add shutdown.
                    None => if seen_any_sse {
                        break;
                    }
                    Some((shutdown_res, _shutdown_rx)) => match shutdown_res {
                        Some(void) => match void {},
                        None => {
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
                path_tx,
                resp_tx,
                shutdown_rx,
            } => {
                match connections.entry(id.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(path_tx);
                    }
                    // TODO: Reply with refusal if session SSE already open
                    _ => (),
                }

                seen_any_sse = true;
                shutdown_futures.push(shutdown_rx.into_future());
                // Should always succeed because resp_rx is owned by handler
                // that just sent NewConnection, is awaiting this message
                // and is only never cancelled
                resp_tx
                    .unbounded_send(disconnect_tx.clone())
                    .expect("SSE handler response channel");
            }
            RouterMessage::FileChange { path } => {
                for path_tx in connections.values() {
                    // Sender should always succeed
                    // because path_rx is returned to us
                    path_tx
                        .unbounded_send(path.clone())
                        .expect("Connection handler channel alive");
                }
            }
        }
    }
    drop(connections);
    drop(disconnect_tx);
    while let Some((_id, _path_rx)) = disconnect_rx.next().await {}
    Ok(())
}

async fn process_sse_channel(
    path_rx: &mut Receiver<OsString>,
    sender: SSESender,
) -> tide::Result {
    while let Some(path) = path_rx.next().await {
        sender
            .send("file_change", &serde_json::to_string(&path)?, None)
            .await?;
    }
    Ok(tide::StatusCode::Ok.into())
}

async fn run() -> Result<(), Box<dyn Error>> {
    let (message_tx, message_rx) = unbounded_channel::<RouterMessage>();
    let mut router = router_handler(message_rx).boxed().fuse();

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

    let mut http_server: tide::Server<()> = tide::new();
    http_server.with(
        tide::sessions::SessionMiddleware::new(
            tide::sessions::MemoryStore::new(),
            b"TODO: Generate at random on first boot and store in config.",
        )
        .with_cookie_name("gmtool.gcsagent"),
    );
    http_server.with(tide::utils::Before(
        |mut request: tide::Request<()>| async move {
            let session = request.session_mut();
            let mut curdir = session
                .get::<Option<std::path::PathBuf>>("curdir")
                .unwrap_or_default();
            if let None = curdir {
                curdir = match std::env::current_dir() {
                    Ok(curdir) => Some(curdir.into()),
                    Err(e) => {
                        eprintln!("Current directory missing {:?}", e);
                        // In the absence of a current directory
                        // hope client gives us an absolute path to work with.
                        None
                    }
                }
            }
            session.insert("curdir", curdir).unwrap();
            request
        },
    ));
    http_server.at("/webui").get(get_webui);
    http_server
        .at("/sse")
        .get(move |req: tide::Request<()>| {
            let message_tx = message_tx.clone();
            async move {
                let resp = tide::sse::upgrade(req, move |req, sender| {
                    let message_tx = message_tx.clone();
                    async move {
                        let (path_tx, mut path_rx) = unbounded_channel();
                        let (resp_tx, mut resp_rx) = unbounded_channel();
                        // TODO: Move shutdown handling into a middleware.
                        let (_shutdown_tx, shutdown_rx) = unbounded_channel();
                        let id = req.session().id().to_string();
                        message_tx
                            .unbounded_send(RouterMessage::NewConnection {
                                id: id.clone(),
                                path_tx,
                                resp_tx,
                                shutdown_rx,
                            })
                            .expect("Message channel live");

                        // next should always be some disconnect_tx because
                        // resp_tx was sent to router which outlives us
                        let disconnect_tx =
                            resp_rx.next().await.expect("Router response");
                        drop(resp_rx);

                        // Inner loop of sse messages
                        let res = process_sse_channel(&mut path_rx, sender).await;
                        // send should always succeed because disconnect_rx is owned
                        // by the router routine that gave us _tx
                        // and that routine only ends after all SSE handlers disconnect
                        disconnect_tx.unbounded_send((id, path_rx))?;
                        res?;
                        Ok(())
                    }
                });
                Ok(resp)
            }
        });
    http_server
        .at("/watch")
        .post(move |mut req: tide::Request<()>| {
            let watcher_ref = Arc::clone(&watcher_ref);
            async move {
                let path: PortableOsString =
                    bincode::deserialize(&req.body_bytes().await?)?;
                watcher_ref.lock_arc().await.watch(
                    OsString::try_from(path).unwrap(),
                    RecursiveMode::NonRecursive,
                )?;
                tide::Result::from(Ok(tide::StatusCode::Ok))
            }
        });
    http_server
        .at("/chdir")
        .post(|mut req: tide::Request<()>| async move {
            let bytes_res = req.body_bytes().await;
            let bytes = match bytes_res {
                Ok(bytes) => bytes,
                Err(e) => {
                    let msg = format!("Body read failed {:?}", e);
                    eprintln!("{}", msg);
                    return Err(tide::Error::from_str(
                        tide::StatusCode::InternalServerError,
                        msg,
                    ))
                    .into();
                }
            };
            let path: PortableOsString = match bincode::deserialize(&bytes) {
                Ok(path) => path,
                Err(e) => {
                    let msg = format!("Body deserialize failed {:?}", e);
                    eprintln!("{}", msg);
                    return Err(tide::Error::from_str(
                        tide::StatusCode::BadRequest,
                        msg,
                    ))
                    .into();
                }
            };
            let path: PathBuf = path.try_into().expect("native OsString");
            let session = req.session_mut();
            let mut curdir =
                session.get::<Option<PathBuf>>("curdir").unwrap_or_default();

            // Update current directory
            let chdir_result = if let Some(ref mut curdir) = curdir {
                curdir.push(path);
                Ok(tide::Body::from(bincode::serialize(
                    &PortableOsString::from(curdir.clone()),
                )?))
            } else if path.is_absolute() {
                curdir = Some(path.clone());
                Ok(tide::Body::from(bincode::serialize(
                    &PortableOsString::from(path),
                )?))
            } else {
                let msg = format!(
                    "Couldn't chdir to {:?}, \
                     path is relative and have no \
                     current directory",
                    path
                );
                eprintln!("{}", msg);
                Err(tide::Error::from_str(
                    tide::StatusCode::PreconditionRequired,
                    msg,
                ))
            };

            session.insert("curdir", curdir)?;

            tide::Result::from(chdir_result)
        });
    http_server
        .at("/lsdir")
        .post(|req: tide::Request<()>| async move {
            let session = req.session();
            // TODO: Deserialise body
            let curdir =
                session.get::<Option<PathBuf>>("curdir").unwrap_or_default();
            let list = match curdir {
                Some(path) => get_dir_file_entries(&path.into()).await?,
                None => {
                    return Err(tide::Error::from_str(
                        tide::StatusCode::PreconditionRequired,
                        "No current directory to list",
                    ))
                }
            };

            tide::Result::Ok(tide::Body::from(bincode::serialize(&list)?))
        });
    http_server
        .at("/read")
        .post(|mut req: tide::Request<()>| async move {
            let path: PortableOsString =
                bincode::deserialize(&req.body_bytes().await?)?;
            let path: OsString = path.try_into().expect("native OsString");
            let file: gcs::FileKind = serde_json::from_str(
                fs::read_to_string(&path).await?.as_str(),
            )?;

            tide::Result::Ok(tide::Body::from(serde_cbor::to_vec(&file)?))
        });

    let http_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let mut http_listener = http_server.bind(http_addr).await?;

    let httpaddrs = http_listener.info();
    println!(
        "Bound server to {:?}",
        httpaddrs
            .iter()
            .map(ListenInfo::to_string)
            .collect::<Vec<String>>()
    );

    let mut urls: Vec<Url> = Vec::new();
    for httpaddr in httpaddrs {
        let mut url = Url::parse_with_params(
            &httpaddr.to_string(),
            &[("agentaddr", httpaddr.to_string())],
        )?;
        url.set_path("/webui");
        urls.push(url);
    }

    // NOTE: Necessarily uses an executor specific spawn
    //       Because a blocking API needs to be used.
    //       Spawning an OS thread an asynchronously sending via unbounded
    //       is not worth the effort to be executor independant
    //       and launch_browser(urls).boxed() is a task that blocks.
    let launch = task::spawn(launch_browser(urls));

    let mut launch_fused = launch.fuse();
    let mut http_listener_accept_fused = http_listener.accept().fuse();
    //let mut rx_fused = rx.fuse();
    loop {
        select! {
            res = http_listener_accept_fused => {
                // http listener accept in principle returns
                // when the connection stops accepting,
                // but that would be in the case of error
                // and as of now tide logs and swallows them
                // Realistically we drop this future for another reason
                break Ok(res?);
            },
            launched = launch_fused => {
                // Launching can fail, and if it does we can't recover
                launched?;
            },
            res = router => {
                break res;
            },
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    task::block_on(run())
}
