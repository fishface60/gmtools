use std::io::Error as IOError;

use futures::{
    channel::mpsc::unbounded as unbounded_channel, prelude::*, select,
};

use async_std::{
    net::{SocketAddr, TcpListener},
    task,
};
use async_tungstenite::{
    accept_async,
    tungstenite::{protocol::Message, Error as TungsteniteError},
};

use http_types::mime;

use notify::{
    event::Event as NotifyEvent, Error as NotifyError, RecommendedWatcher,
    RecursiveMode, Result as NotifyResult, Watcher,
};

use tide::{self, prelude::*, Response};

use url::{self, Url};

use webbrowser;

#[derive(Debug)]
enum GCSAgentError {
    IOError(IOError),
    NotifyError(NotifyError),
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

impl From<TungsteniteError> for GCSAgentError {
    fn from(error: TungsteniteError) -> Self {
        GCSAgentError::WebSocketError(error)
    }
}

async fn handle(_req: tide::Request<()>) -> tide::Result {
    // TODO: Need to return this as an HTML document rather than text
    Ok(Response::builder(200)
        .body(include_str!(env!("WEBUI_HTML_PATH")))
        .content_type(mime::HTML)
        .build())
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
        http_server.at("/").get(handle);

        let http_addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let mut http_listener = http_server.bind(http_addr).await?;

        let httpaddrs = http_listener.info().clone();

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

                if let Ok(_) = webbrowser::open(&url.as_str()) {
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

    loop {
        select! {
            msg = fused_socket_reader.select_next_some() => {
                match msg? {
                    Message::Close(_) => {
                        break;
                    },
                    Message::Text(path) => {
                        watcher.watch(path, RecursiveMode::Recursive)?;
                    }
                    msg => {
                        return Err(GCSAgentError::UnexpectedMessage(msg));
                    },
                }
            },
            path = rx.next() => {
                // TODO: Send both path and contents?
                let resp = format!("{} changed",
                                   path.unwrap().to_str().unwrap());
                ws_writer.send(Message::Text(resp)).await?;
            },
            complete => break,
        }
    }

    Ok(())
}

fn main() -> Result<(), GCSAgentError> {
    task::block_on(run())
}
