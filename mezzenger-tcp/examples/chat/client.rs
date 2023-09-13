use anyhow::Result;
use futures::{FutureExt, SinkExt, StreamExt};
use kodec::binary::Codec;
use mezzenger::{Messages, Receive};
use mezzenger_tcp::Transport;
use parity_tokio_ipc::Endpoint;
use rustyline_async::{Readline, ReadlineEvent};
use serde::{Deserialize, Serialize};
use std::io::Write;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    select,
};

use crate::server;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Init { user_name: String },
    Message { content: String },
}

pub async fn run(ipc: bool, address: &str, path: &str) -> Result<()> {
    println!("Connecting to server...");
    if ipc {
        run_inner(Endpoint::connect(&path).await?).await
    } else {
        run_inner(TcpStream::connect(address).await?).await
    }
}

async fn run_inner<S>(stream: S) -> Result<()>
where
    S: AsyncWrite + AsyncRead,
{
    let codec = Codec::default();
    let (mut sender, mut receiver) =
        Transport::<_, Codec, server::Message, Message>::new(stream, codec).split();
    println!("Connected.");

    let (mut readline, mut stdout) = Readline::new("> ".to_string())?;
    writeln!(stdout, "Type your name:")?;

    let name = if let ReadlineEvent::Line(name) = readline.readline().await? {
        name
    } else {
        println!("Exiting...");
        return Ok(());
    };
    sender
        .send(Message::Init {
            user_name: name.clone(),
        })
        .await?;

    let init_message = receiver.receive().await?;
    match init_message {
        server::Message::Init { name_already_taken } => {
            if name_already_taken {
                panic!("name already taken");
            }
        }
        _ => panic!("unexpected message received"),
    }
    writeln!(stdout, "Hello {name}.")?;

    readline.flush()?;

    let (mut readline, mut stdout) = Readline::new("> ".to_string())?;
    let mut stdout_clone = stdout.clone();
    let mut message_stream = receiver.messages_with_error_callback(move |error| {
        let _ = writeln!(
            stdout_clone,
            "Error occurred while receiving message: {error}."
        );
    });

    loop {
        select! {
            message = message_stream.next() => {
                if let Some(message) = message {
                    match message {
                        server::Message::UserConnected { user_name } => {
                            writeln!(stdout, "New user connected: <{user_name}>.")?;
                        },
                        server::Message::UserDisconnected { user_name } => {
                            writeln!(stdout, "User <{user_name}> left.")?;
                        },
                        server::Message::Message { user_name, content } => {
                            writeln!(stdout, "<{user_name}> {content}")?;
                        },
                        _ => panic!("unexpected message received"),
                    }
                } else {
                    writeln!(stdout, "Server disconnected.")?;
                    writeln!(stdout, "Exiting...")?;
                    break;
                }
            },
            command = readline.readline().fuse() => match command {
                Ok(event) => {
                    match event {
                        ReadlineEvent::Line(line) => {
                            let message = Message::Message { content: line.to_string() };
                            sender.send(message).await?;
                        },
                        ReadlineEvent::Eof | ReadlineEvent::Interrupted => {
                            writeln!(stdout, "Exiting...")?;
                            break;
                        }
                    }
                },
                Err(error) => {
                    writeln!(stdout, "Error occurred while handling command: {error}")?;
                    writeln!(stdout, "Exiting...")?;
                    break;
                },
            },
        }
    }
    readline.flush()?;

    Ok(())
}
