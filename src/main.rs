use std::io::Error as IOError;

use futures::{
    channel::mpsc::unbounded as unbounded_channel, prelude::*, select,
};

use async_std::{net::TcpListener, task::block_on};
use async_tungstenite::{
    accept_async,
    tungstenite::{protocol::Message, Error as TungsteniteError},
};

use notify::{
    event::Event as NotifyEvent, Error as NotifyError, RecommendedWatcher,
    RecursiveMode, Result as NotifyResult, Watcher,
};

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

async fn run() -> Result<(), GCSAgentError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

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

    // TODO: Navigate to a page that connects to this socket.
    println!("{}", addr);

    let ws_stream = loop {
        let (stream, addr) = listener.accept().await?;
        println!("Got peer connection from {}", addr);

        match accept_async(stream).await {
            Ok(ws_stream) => break ws_stream,
            Err(e) => println!("Peer failed to connect: {}", e),
        };
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
    block_on(run())
}
