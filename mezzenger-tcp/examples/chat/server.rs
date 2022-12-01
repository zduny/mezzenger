use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{bail, Result};
use futures::{pin_mut, FutureExt, SinkExt, StreamExt};
use kodec::binary::Codec;
use mezzenger::Receive;
use mezzenger_tcp::Transport;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    select, spawn,
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        RwLock,
    },
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{error, info, Level};

use crate::client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Init { name_already_taken: bool },
    UserConnected { user_name: String },
    UserDisconnected { user_name: String },
    Message { user_name: String, content: String },
}

#[derive(Debug)]
struct User {
    sender: UnboundedSender<Message>,
}

impl User {
    fn new(sender: UnboundedSender<Message>) -> Self {
        User { sender }
    }
}

#[derive(Debug, Default)]
struct State {
    users: HashMap<String, User>,
}

pub async fn run(address: &str) -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Server running!");
    let state = Arc::new(RwLock::new(State::default()));

    let listener = TcpListener::bind(&address).await?;
    info!("Listening at {}...", address);

    let break_signal = tokio::signal::ctrl_c().fuse();
    pin_mut!(break_signal);
    loop {
        select! {
            listener_result = listener.accept() => {
                let (stream, address) = listener_result?;
                let state = state.clone();
                spawn(async move {
                    tracing::debug!("accepted connection");
                    if let Err(error) = user_connected(stream, address, state).await {
                        error!("Error occurred: {error}");
                    }
                });
            },
            break_result = &mut break_signal => {
                break_result.expect("failed to listen for event");
                break;
            }
        }
    }
    info!("Shutting down...");

    Ok(())
}

async fn user_connected(
    stream: TcpStream,
    _address: SocketAddr,
    state: Arc<RwLock<State>>,
) -> Result<()> {
    let codec = Codec::default();
    let (mut sender, mut receiver) =
        Transport::<_, Codec, client::Message, Message>::new(stream, codec).split();

    let init_message = receiver.receive().await?;
    match init_message {
        client::Message::Init { user_name } => {
            let (user_sender, user_receiver) = unbounded_channel();
            let mut user_receiver = UnboundedReceiverStream::new(user_receiver);

            let user = User::new(user_sender);
            {
                let mut state = state.write().await;
                if state.users.contains_key(&user_name) {
                    sender
                        .send(Message::Init {
                            name_already_taken: true,
                        })
                        .await?;
                    bail!("user with that name already exists");
                } else {
                    state.users.insert(user_name.clone(), user);
                    sender
                        .send(Message::Init {
                            name_already_taken: false,
                        })
                        .await?;
                }
            }

            info!("User <{user_name}> connected.");

            let message = Message::UserConnected {
                user_name: user_name.clone(),
            };
            for (name, user) in state.read().await.users.iter() {
                if &user_name != name {
                    let _ = user.sender.send(message.clone());
                }
            }

            let user_name_clone = user_name.clone();
            spawn(async move {
                while let Some(message) = user_receiver.next().await {
                    let result = sender.send(message).await;
                    if let Err(error) = result {
                        error!("Failed to send message to user: name: {user_name_clone}, error: {error}.");
                    }
                }
            });

            while let Some(result) = receiver.next().await {
                let msg = match result {
                    Ok(msg) => msg,
                    Err(error) => {
                        error!("Failed to receive message from user: {user_name}, error: {error}.");
                        break;
                    }
                };
                user_message(user_name.clone(), msg, &state).await?;
            }

            user_disconnected(user_name, &state).await;
        }
        _ => bail!("unexpected message received"),
    }

    Ok(())
}

async fn user_message(
    name: String,
    message: client::Message,
    state: &Arc<RwLock<State>>,
) -> Result<()> {
    match message {
        client::Message::Message { content } => {
            let message = Message::Message {
                user_name: name,
                content,
            };
            for user in state.read().await.users.values() {
                let _ = user.sender.send(message.clone());
            }
        }
        _ => bail!("unexpected message received"),
    }

    Ok(())
}

async fn user_disconnected(name: String, state: &Arc<RwLock<State>>) {
    info!("User <{name}> disconnected.");

    let message = Message::UserDisconnected {
        user_name: name.clone(),
    };
    for (user_name, user) in state.read().await.users.iter() {
        if &name != user_name {
            let _ = user.sender.send(message.clone());
        }
    }

    state.write().await.users.remove(&name);
}
