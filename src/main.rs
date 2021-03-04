#![allow(clippy::single_component_path_imports)]

use std::io::Error as IOError;

use futures::{
    channel::mpsc::unbounded as unbounded_channel, prelude::*, select,
    stream::SplitSink,
};

use async_std::{
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    task,
};
use async_tungstenite::{
    accept_async,
    tungstenite::{protocol::Message, Error as TungsteniteError},
    WebSocketStream,
};

use bincode::{self, Error as BincodeError};

use http_types::mime;

use notify::{
    event::Event as NotifyEvent, Error as NotifyError, RecommendedWatcher,
    RecursiveMode, Result as NotifyResult, Watcher,
};

use tide::{self, prelude::*, Response};

use url::{self, Url};

use webbrowser;

use gmtool_common::{FileEntry, GCSAgentMessage, WebUIMessage};

#[derive(Debug)]
enum GCSAgentError {
    IOError(IOError),
    NotifyError(NotifyError),
    RequestWatchError(BincodeError),
    SerializeError(Box<bincode::ErrorKind>),
    UnexpectedMessage(Message),
    WebSocketError(TungsteniteError),
}

impl From<IOError> for GCSAgentError {
    fn from(error: IOError) -> Self {
        GCSAgentError::IOError(error)
    }
}

impl From<NotifyError> for GCSAgentError {
    fn from(error: NotifyError) -> Self {
        GCSAgentError::NotifyError(error)
    }
}

impl From<Box<bincode::ErrorKind>> for GCSAgentError {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        GCSAgentError::SerializeError(error)
    }
}

impl From<TungsteniteError> for GCSAgentError {
    fn from(error: TungsteniteError) -> Self {
        GCSAgentError::WebSocketError(error)
    }
}

async fn handle_http_request(_req: tide::Request<()>) -> tide::Result {
    // TODO: Need to return this as an HTML document rather than text
    Ok(Response::builder(200)
        .body(include_str!(env!("WEBUI_HTML_PATH")))
        .content_type(mime::HTML)
        .build())
}

enum DirSendError {
    SendError(TungsteniteError),
    SerializeError(Box<bincode::ErrorKind>),
}

impl From<DirSendError> for GCSAgentError {
    fn from(error: DirSendError) -> Self {
        match error {
            DirSendError::SendError(e) => GCSAgentError::WebSocketError(e),
            DirSendError::SerializeError(e) => GCSAgentError::SerializeError(e),
        }
    }
}

impl From<TungsteniteError> for DirSendError {
    fn from(error: TungsteniteError) -> Self {
        DirSendError::SendError(error)
    }
}

impl From<Box<bincode::ErrorKind>> for DirSendError {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        DirSendError::SerializeError(error)
    }
}

async fn get_dir_file_entries(
    path: &PathBuf,
) -> Result<Vec<FileEntry>, IOError> {
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut dents = path.read_dir().await?;
    while let Some(dirent) = dents.next().await {
        let dirent = dirent?;
        let metadata = dirent.metadata().await?;
        let name = dirent.file_name();
        if metadata.is_dir() {
            match name.into_string() {
                Ok(s) => entries.push(FileEntry::Directory(s)),
                Err(_name) => (),
            }
            continue;
        }
        if metadata.is_file() {
            match name.into_string() {
                Ok(s) => {
                    if s.ends_with(".gcs") {
                        entries.push(FileEntry::GCSFile(s));
                    }
                }
                Err(_name) => (),
            }
        }
    }
    Ok(entries)
}

async fn send_dir_gcs_files(
    path: &PathBuf,
    ws_writer: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
) -> Result<(), DirSendError> {
    let msg =
        match get_dir_file_entries(path).await {
            Ok(entries) => {
                match path.to_str() {
                    Some(s) => GCSAgentMessage::DirectoryChangeNotification(
                        Ok((s.to_string(), entries)),
                    ),
                    None => {
                        // Having no path is an acceptable thing to ignore,
                        // so having an unencodable one should be too.
                        return Ok(());
                    }
                }
            }
            Err(e) => GCSAgentMessage::DirectoryChangeNotification(Err(
                format!("{:?}", e),
            )),
        };
    ws_writer
        .send(Message::Binary(bincode::serialize(&msg)?))
        .await?;
    Ok(())
}

// TODO: When std::ops::ControlFlow leaves nightly, use that
enum ControlFlow {
    Continue,
    Break,
}

async fn handle_socket_message(
    msg: Result<Message, TungsteniteError>,
    curdir: &mut Option<PathBuf>,
    ws_writer: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    watcher: &mut RecommendedWatcher,
) -> Result<ControlFlow, GCSAgentError> {
    match msg? {
        Message::Close(_) => Ok(ControlFlow::Break),
        Message::Binary(bytes) => {
            let msg = match bincode::deserialize(&bytes) {
                Ok(msg) => msg,
                Err(e) => {
                    println!("Err: {:?}", e);
                    return Ok(ControlFlow::Continue);
                }
            };
            match msg {
                WebUIMessage::RequestChDir(ref path) => {
                    // Update current directory
                    let chdir_result = if let Some(ref mut curdir) = curdir {
                        curdir.push(path);
                        Ok(())
                    } else {
                        let pathbuf = PathBuf::from(path);
                        if pathbuf.is_absolute() {
                            *curdir = Some(pathbuf);
                            Ok(())
                        } else {
                            let msg = format!("Couldn't chdir to {:?}, \
                                               path is relative and have no \
                                               current directory", path);
                            Err(msg)
                        }
                    };

                    let msg = GCSAgentMessage::RequestChDirResult(chdir_result);
                    ws_writer
                        .send(Message::binary(bincode::serialize(&msg)?))
                        .await?;

                    if let GCSAgentMessage::RequestChDirResult(chdir_result) =
                        msg
                    {
                        if chdir_result.is_err() {
                            return Ok(ControlFlow::Continue);
                        }
                    }

                    if let Some(ref mut curdir) = curdir {
                        send_dir_gcs_files(&curdir, ws_writer).await?;
                    }
                    Ok(ControlFlow::Continue)
                }
                WebUIMessage::RequestWatch(path) => {
                    let mut filepath = PathBuf::new();
                    if let Some(ref curdir) = curdir {
                        filepath.push(curdir);
                    }
                    filepath.push(path);
                    if filepath.is_relative() {
                        println!("Can't request relative path {:?}", filepath);
                        return Ok(ControlFlow::Continue);
                    }
                    let res = match watcher
                        .watch(filepath, RecursiveMode::Recursive)
                    {
                        Ok(_) => Ok(()),
                        Err(e) => Err(format!("{:?}", e)),
                    };
                    // TODO: Send both path and contents?
                    let resp = GCSAgentMessage::RequestWatchResult(res);
                    match bincode::serialize(&resp) {
                        Ok(bytes) => {
                            ws_writer.send(Message::Binary(bytes)).await?;
                            Ok(ControlFlow::Continue)
                        }
                        Err(e) => {
                            return Err(GCSAgentError::RequestWatchError(e));
                        }
                    }
                }
            }
        }
        msg => {
            return Err(GCSAgentError::UnexpectedMessage(msg));
        }
    }
}

async fn run() -> Result<(), GCSAgentError> {
    let ws_listener = TcpListener::bind("127.0.0.1:0").await?;
    let ws_addr = ws_listener.local_addr()?;

    let (tx, mut rx) = unbounded_channel();

    // Spawn file watcher thread
    let mut watcher: RecommendedWatcher =
        Watcher::new_immediate(move |res: NotifyResult<NotifyEvent>| {
            // TODO: How can we do error handling
            // when this callback doesn't have a Result return type?
            let event = res.unwrap();
            for path in event.paths {
                println!(
                    "Got notify event on path: {}",
                    path.to_str().unwrap()
                );
                tx.unbounded_send(path).unwrap();
            }
        })?;

    let ws_stream = {
        let mut http_server: tide::Server<()> = tide::new();
        http_server.at("/").get(handle_http_request);

        let http_addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let mut http_listener = http_server.bind(http_addr).await?;

        let httpaddrs = http_listener.info();

        let launch = task::spawn(async move {
            // TODO: Error handling for _why_ launch failed.
            for httpaddr in httpaddrs {
                let url = if let Ok(url) = Url::parse_with_params(
                    &httpaddr.to_string(),
                    &[("agentaddr", ws_addr.to_string())],
                ) {
                    url
                } else {
                    continue;
                };

                if webbrowser::open(&url.as_str()).is_ok() {
                    return true;
                }
            }
            false
        });

        let mut launch_fused = launch.fuse();
        let mut http_listener_accept_fused = http_listener.accept().fuse();
        loop {
            select! {
                res = http_listener_accept_fused => {
                    // We're expecting to drop this rather than it return.
                    // If it did so, it was probably an error.
                    // TODO: Replace with an Error value
                    assert!(res.is_err());
                },
                launched = launch_fused => {
                    // Launching can fail, and if it does we can't recover
                    // TODO: Replace with an Error value
                    assert!(launched);
                },
                // TODO: Should be some kind of iterator,
                // so we can accept multiple.
                res = ws_listener.accept().fuse() => {
                    let (stream, addr) = res?;
                    println!("Got peer connection from {}", addr);
                    match accept_async(stream).await {
                        Ok(ws_stream) => break ws_stream,
                        Err(e) => println!("Peer failed to connect: {}", e),
                    };
                },
            }
        }
    };

    let (mut ws_writer, ws_reader) = ws_stream.split();
    let mut fused_socket_reader = ws_reader.fuse();

    // Introduce ourself with current dir
    let mut curdir: Option<PathBuf> = match std::env::current_dir() {
        Ok(curdir) => Some(curdir.into()),
        Err(_) => {
            // TODO: Log current dir doesn't exist
            // In the absence of a current directory
            // hope client gives us an absolute path to work with.
            None
        }
    };

    if let Some(curdir) = &curdir {
        send_dir_gcs_files(curdir, &mut ws_writer).await?;
    }

    loop {
        select! {
            msg = fused_socket_reader.select_next_some() => {
                match handle_socket_message(msg, &mut curdir, &mut ws_writer,
                                            &mut watcher).await? {
                    ControlFlow::Continue => continue,
                    ControlFlow::Break => break,
                }
            },
            path = rx.next() => {
                // TODO: Send both path and contents?
                let resp = GCSAgentMessage::FileChangeNotification(path.unwrap().to_str().unwrap().to_string());
                let serialised = bincode::serialize(&resp);
                // TODO: Less silent serialise error.
                if let Ok(bytes) = serialised {
                    ws_writer.send(Message::Binary(bytes)).await?;
                }
            },
            complete => break,
        }
    }

    Ok(())
}

fn main() -> Result<(), GCSAgentError> {
    task::block_on(run())
}
