use std::borrow::Cow;
use std::env::args;

use futures::{
    channel::mpsc::{unbounded as unbounded_channel, UnboundedSender},
    select, SinkExt, StreamExt,
};

use async_std::{
    io::stdin,
    task::{block_on, spawn},
};
use async_tungstenite::{
    async_std::connect_async,
    tungstenite::protocol::{
        frame::{coding::CloseCode, CloseFrame},
        Message,
    },
};

async fn read_stdin(tx: UnboundedSender<Message>) {
    let stdin = stdin();
    let mut line = String::new();
    while let Ok(size) = stdin.read_line(&mut line).await {
        if size == 0 {
            break;
        }
        tx.unbounded_send(Message::Text(line.trim_end().to_string()))
            .unwrap();
        line.truncate(0);
    }
}

async fn run() {
    let connect_addr = args().nth(1).unwrap();

    let (stdin_tx, mut stdin_rx) = unbounded_channel();
    spawn(read_stdin(stdin_tx));

    let (ws_stream, _) = connect_async(&connect_addr).await.unwrap();
    let (mut write, read) = ws_stream.split();

    let mut fused_read = read.fuse();
    loop {
        select! {
            msg = fused_read.select_next_some() => {
                match msg.unwrap() {
                    Message::Close(_) => {
                        break;
                    },
                    Message::Text(path) => {
                        println!("Got message: {}", path);
                    }
                    msg => {
                        panic!("Unexpected message: {}", msg);
                    },
                }
            },
            msg = stdin_rx.next() => {
                match msg {
                    Some(msg) => {
                        write.send(msg).await.unwrap();
                    },
                    None => break,
                }
            },
            complete => break,
        }
    }

    let read = fused_read.into_inner();
    let mut ws_stream = read.reunite(write).unwrap();

    let close_frame = CloseFrame {
        code: CloseCode::Normal,
        reason: Cow::from("Application closed"),
    };
    ws_stream.close(Some(close_frame)).await.unwrap();
}

fn main() {
    block_on(run())
}
