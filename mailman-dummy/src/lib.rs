//! Transport storing messages in local queue.
//!
//! It does not send messages anywhere (to other side of transport) - messages 'received'
//! from this transport are the same messages that were 'sent' into it locally.
//!
//! **NOTE**: do NOT use this transport to send data between threads or async tasks,
//! use appropriate channels instead as they will most likely be much more performant.

#![feature(generic_associated_types)]

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(test)]
mod tests {
    use crate::Transport;
    use futures::StreamExt;
    use mailman::{Close, Error, Receive, Send};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Message {
        Integer(i32),
        String(String),
        Symbol,
    }

    async fn test_send_and_receive() {
        let message_0 = Message::Integer(2);
        let message_1 = Message::String("Hello World!".to_string());
        let message_2 = Message::Symbol;

        let transport = Transport::new();

        transport.send(&message_0).await.unwrap();
        transport.send(&message_1).await.unwrap();
        transport.send(&message_2).await.unwrap();

        assert_eq!(message_0, transport.receive().await.unwrap());
        assert_eq!(message_1, transport.receive().await.unwrap());
        assert_eq!(message_2, transport.receive().await.unwrap());

        transport.send(&message_0).await.unwrap();
        transport.send(&message_1).await.unwrap();

        transport.close().await;

        assert_eq!(Error::Closed, transport.send(&message_2).await.unwrap_err());

        assert_eq!(message_0, transport.receive().await.unwrap());
        assert_eq!(message_1, transport.receive().await.unwrap());
        assert_eq!(Error::Closed, transport.receive().await.unwrap_err());
    }

    async fn test_stream() {
        let message_0 = Message::Integer(2);
        let message_1 = Message::String("Hello World!".to_string());
        let message_2 = Message::Symbol;

        let transport = Transport::new();
        let stream = transport.stream();

        transport.send(&message_0).await.unwrap();
        transport.send(&message_1).await.unwrap();
        transport.send(&message_2).await.unwrap();

        transport.close().await;

        assert_eq!(
            vec![message_0, message_1, message_2],
            stream.collect::<Vec<Message>>().await
        )
    }

    async fn test_all() {
        test_send_and_receive().await;
        test_stream().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test() {
        test_all().await;
    }

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test() {
        test_all().await;
    }
}
