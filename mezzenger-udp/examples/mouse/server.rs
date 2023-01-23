use std::time::Duration;

use anyhow::Result;
use device_query::{DeviceQuery, DeviceState};
use futures::{pin_mut, FutureExt};
use kodec::binary::Codec;
use mezzenger_udp::Transport;
use mezzenger_utils::numbered;
use serde::{Deserialize, Serialize};
use tokio::{net::UdpSocket, select, signal::ctrl_c, time::interval};
use tracing::{info, Level};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub mouse_x: i32,
    pub mouse_y: i32,
}

pub async fn run(address: &str) -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Server running!");

    let udp_socket = UdpSocket::bind("0.0.0.0:4321").await?;
    udp_socket.set_broadcast(true)?;
    let codec = Codec::default();
    let mut transport =
        Transport::<_, Codec, (), numbered::Wrapper<u64, Message>>::new(udp_socket, codec);

    let device_state = DeviceState::new();

    let mut interval = interval(Duration::from_millis(50));
    let break_signal = ctrl_c().fuse();
    pin_mut!(break_signal);
    let mut message_number = 0;
    loop {
        select! {
            _tick = interval.tick() => {
                let mouse_state = device_state.get_mouse();
                let (x, y) = mouse_state.coords;
                let message = Message { mouse_x: x, mouse_y: y };
                let message = numbered::Wrapper { number: message_number, wrapped: message };
                transport.send_to(message, address).await?;
                message_number = message_number.wrapping_add(1);
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
