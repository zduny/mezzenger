use std::{
    io::{stdout, Write},
    time::{Duration, Instant},
};

use anyhow::Result;
use futures::{pin_mut, FutureExt, StreamExt};
use kodec::binary::Codec;
use mezzenger::Messages;
use mezzenger_udp::Transport;
use mezzenger_utils::{numbered, LatestOnly};
use tokio::{net::UdpSocket, select, signal::ctrl_c, time::interval};

use crate::server;

const SERVER_DOWN_TIMEOUT: Duration = Duration::from_secs(1);

pub async fn run() -> Result<()> {
    let udp_socket = UdpSocket::bind("0.0.0.0:1234").await?;
    let codec = Codec::default();
    let transport =
        Transport::<_, Codec, numbered::Wrapper<u64, server::Message>, ()>::new(udp_socket, codec);
    let transport = LatestOnly::new(transport).into_unwrapping();

    print!("Connecting...");
    stdout().flush()?;

    let mut messages = transport.messages_with_error_callback(|_| println!("Error!!!"));

    let mut server_down = false;
    let mut last_time_received = Instant::now();

    let mut interval = interval(Duration::from_millis(500));
    let break_signal = ctrl_c().fuse();
    pin_mut!(break_signal);
    loop {
        select! {
            message = messages.next() => {
                if let Some(message) = message {
                    server_down = false;
                    last_time_received = Instant::now();
                    print!("\r                                                            ");
                    print!("\rCurrent server mouse position: [ x: {}, y: {} ]", message.mouse_x, message.mouse_y);
                    stdout().flush()?;
                } else {
                    break;
                }
            },
            _ = interval.tick() => {
                let duration = Instant::now() - last_time_received;
                if duration > SERVER_DOWN_TIMEOUT && !server_down {
                    server_down = true;
                    print!("\r                                                            ");
                    print!("\rServer seems to be down...");
                    stdout().flush()?;
                }
            }
            break_result = &mut break_signal => {
                break_result.expect("failed to listen for event");
                break;
            },
        }
    }

    println!("\nShutting down...");

    Ok(())
}
