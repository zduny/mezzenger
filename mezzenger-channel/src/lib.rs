//! Transport for communication over [futures](https://github.com/rust-lang/futures-rs) channels.
//!
//! Useful for testing and debugging.
//!
//! See [repository](https://github.com/zduny/mezzenger) for more info.
//!
//! ## Example
//!
//! ```ignore
//! let (mut left, mut right) = transports();
//!
//! left.send("Hello World!").await.unwrap();
//! right.send(123).await.unwrap();
//!
//! use mezzenger::Receive;
//! assert_eq!(right.receive().await.unwrap(), "Hello World!");
//! assert_eq!(left.receive().await.unwrap(), 123);
//! ```

use std::{
    fmt::Display,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{unbounded, SendError, UnboundedReceiver, UnboundedSender},
    stream::{Fuse, FusedStream},
    Sink, Stream, StreamExt,
};
use pin_project::pin_project;

#[derive(Debug)]
pub enum Error {
    ChannelIsFull,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "channel is full")
    }
}

impl std::error::Error for Error {}

/// Transport for communication over [futures](https://github.com/rust-lang/futures-rs) channels.
#[pin_project]
pub struct Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
    #[pin]
    receiver: Fuse<Receiver>,
    #[pin]
    sender: Sender,
    _incoming: PhantomData<Incoming>,
    _outgoing: PhantomData<Outgoing>,
}

impl<Receiver, Sender, Incoming, Outgoing> Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
    /// Create new transport wrapping provided sender and receiver.
    pub fn new(sender: Sender, receiver: Receiver) -> Self {
        Transport {
            receiver: receiver.fuse(),
            sender,
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }
}

impl<Receiver, Sender, Incoming, Outgoing> Sink<Outgoing>
    for Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
    type Error = mezzenger::Error<Error>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.sender.poll_ready(cx).map_err(map_error)
    }

    fn start_send(self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        let me = self.project();
        me.sender.start_send(item).map_err(map_error)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.sender.poll_flush(cx).map_err(map_error)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.sender.poll_close(cx).map_err(map_error)
    }
}

impl<Receiver, Sender, Incoming, Outgoing> Stream
    for Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
    type Item = Result<Incoming, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();
        me.receiver
            .poll_next(cx)
            .map(|item_option| item_option.map(Ok))
    }
}

fn map_error(error: SendError) -> mezzenger::Error<Error> {
    if error.is_full() {
        mezzenger::Error::Other(Error::ChannelIsFull)
    } else {
        mezzenger::Error::Closed
    }
}

impl<Receiver, Sender, Incoming, Outgoing> FusedStream
    for Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
    fn is_terminated(&self) -> bool {
        self.receiver.is_terminated()
    }
}

impl<Receiver, Sender, Incoming, Outgoing> mezzenger::Reliable
    for Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
}

impl<Receiver, Sender, Incoming, Outgoing> mezzenger::Order
    for Transport<Receiver, Sender, Incoming, Outgoing>
where
    Receiver: Stream<Item = Incoming>,
    Sender: Sink<Outgoing, Error = SendError>,
{
}

/// Create two transports over two unbounded channels.
#[allow(clippy::type_complexity)]
pub fn transports<Incoming, Outgoing>() -> (
    Transport<UnboundedReceiver<Incoming>, UnboundedSender<Outgoing>, Incoming, Outgoing>,
    Transport<UnboundedReceiver<Outgoing>, UnboundedSender<Incoming>, Outgoing, Incoming>,
) {
    let (left_sender, right_receiver) = unbounded();
    let (right_sender, left_receiver) = unbounded();

    let left = Transport::new(left_sender, left_receiver);
    let right = Transport::new(right_sender, right_receiver);

    (left, right)
}

#[cfg(test)]
mod tests {
    use futures::{stream, SinkExt, StreamExt};

    use mezzenger::{Messages, Receive};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    use crate::transports;

    async fn test_stream_inner() {
        let (mut left, right) = transports::<String, u32>();

        left.send_all(&mut stream::iter(vec![1, 2, 3].into_iter().map(Ok)))
            .await
            .unwrap();
        drop(left);

        assert_eq!(right.messages().collect::<Vec<u32>>().await, vec![1, 2, 3]);
    }

    async fn test_transport_inner() {
        let (mut left, mut right) = transports();

        left.send("Hello World!".to_string()).await.unwrap();
        left.send("Hello World again!".to_string()).await.unwrap();
        right.send(128).await.unwrap();
        right.send(1).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), "Hello World!");
        assert_eq!(right.receive().await.unwrap(), "Hello World again!");
        assert_eq!(left.receive().await.unwrap(), 128);
        assert_eq!(left.receive().await.unwrap(), 1);
    }

    async fn test_unit_message_inner() {
        let (mut left, mut right) = transports();

        left.send(()).await.unwrap();
        left.send(()).await.unwrap();
        right.send(()).await.unwrap();
        right.send(()).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), ());
        assert_eq!(right.receive().await.unwrap(), ());
        assert_eq!(left.receive().await.unwrap(), ());
        assert_eq!(left.receive().await.unwrap(), ());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_transport() {
        test_transport_inner().await
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_unit_message() {
        test_unit_message_inner().await
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_stream() {
        test_stream_inner().await;
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_transport() {
        test_transport_inner().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_unit_message() {
        test_unit_message_inner().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_stream() {
        test_stream_inner().await
    }
}
