//! Transport for communication over [tokio](https://tokio.rs/)
//! UDP implementation.
//!
//! **NOTE**: This transport inherits UDP properties:
//! - it is **unreliable** - messages are not guaranteed to reach destination,
//! - it is **unordered** - messages may arrive at destination out of order, also they
//! may be duplicated (the same message may arrive at destination twice or more times).
//! - message size is limited to datagram size - sending may result in error if encoded
//! message is too large.
//!
//! See [repository](https://github.com/zduny/mezzenger) for more info.
//!
//! ## Example
//!
//! ```ignore
//! let udp_socket = UdpSocket::bind("127.0.0.1:8080").await?;
//! udp_socket.connect(remote_address).await?;
//!
//! use kodec::binary::Codec;
//! let mut transport: Transport<_, Codec, i32, String> =
//!     Transport::new(udp_socket, Codec::default());
//!
//! use mezzenger::Receive;
//! let integer = transport.receive().await?;
//!
//! transport.send("Hello World!".to_string()).await?;
//! ```

use futures::{future::poll_fn, stream::FusedStream, Sink, SinkExt, Stream};
use kodec::{Decode, Encode};
use pin_project::pin_project;
use serde::Serialize;
use std::{
    borrow::Borrow,
    collections::VecDeque,
    fmt::{Debug, Display},
    io::ErrorKind,
    marker::PhantomData,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::ReadBuf,
    net::{ToSocketAddrs, UdpSocket},
};

#[derive(Debug)]
pub enum Error<SerializationError, DeserializationError> {
    SendingError,
    SerializationError(SerializationError),
    DeserializationError(DeserializationError),
    IoError(tokio::io::Error),
}

impl<SerializationError, DeserializationError> Display
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Display,
    DeserializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SendingError => write!(f, "not all bytes were sent"),
            Error::SerializationError(error) => write!(f, "failed to serialize message: {error}"),
            Error::DeserializationError(error) => {
                write!(f, "failed to deserialize message: {error}")
            }
            Error::IoError(error) => write!(f, "IO error occurred: {error}"),
        }
    }
}

impl<SerializationError, DeserializationError> std::error::Error
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Debug + Display,
    DeserializationError: Debug + Display,
{
}

/// Transport over [tokio](https://tokio.rs/)'s UDP implementation.
///
/// Wraps over [tokio::net::UdpSocket].
///
/// **NOTE**: This transport inherits UDP properties:
/// - it is **unreliable** - messages are NOT guaranteed to reach destination,
/// - it is **unordered** - messages may arrive at destination out of order, also they
/// may be duplicated (the same message may arrive at destination twice or more times).
/// - message size is limited to datagram size - sending may result in error if encoded
/// message is too large.
#[pin_project]
pub struct Transport<U, Codec, Incoming, Outgoing>
where
    U: Borrow<UdpSocket>,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    udp_socket: Option<U>,
    codec: Codec,
    send_queue: VecDeque<Outgoing>,
    send_buffer: Vec<u8>,
    messages_to_send: usize,
    receive_buffer: Vec<u8>,
    _incoming: PhantomData<Incoming>,
}

impl<U, Codec, Incoming, Outgoing> Transport<U, Codec, Incoming, Outgoing>
where
    U: Borrow<UdpSocket>,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    /// Create new transport wrapping a provided `[tokio::net::UdpSocket]`.
    pub fn new(udp_socket: U, codec: Codec) -> Self {
        Transport {
            udp_socket: Some(udp_socket),
            codec,
            send_queue: VecDeque::new(),
            send_buffer: vec![],
            messages_to_send: 0,
            receive_buffer: vec![0; 65536],
            _incoming: PhantomData,
        }
    }

    /// Send message to address.
    pub async fn send_to<A: ToSocketAddrs>(
        &mut self,
        message: Outgoing,
        target: A,
    ) -> Result<(), mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>>
    {
        self.flush().await?;
        if let Some(udp_socket) = &self.udp_socket {
            self.codec
                .encode(&mut self.send_buffer, &message)
                .map_err(
                    Error::<<Codec as Encode>::Error, <Codec as Decode>::Error>::SerializationError,
                )
                .map_err(mezzenger::Error::Other)?;
            udp_socket
                .borrow()
                .send_to(&self.send_buffer, target)
                .await
                .map_err(Error::<<Codec as Encode>::Error, <Codec as Decode>::Error>::IoError)
                .map_err(mezzenger::Error::Other)?;
            Ok(())
        } else {
            Err(mezzenger::Error::Closed)
        }
    }

    /// Receive single message.
    ///
    /// Returns a pair of incoming message and its origin address.
    pub async fn receive_from(
        &mut self,
    ) -> Result<
        (Incoming, SocketAddr),
        mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>,
    > {
        if self.udp_socket.is_some() {
            let result = poll_fn(|cx| self.poll_recv_from(cx)).await;
            if let Some(result) = result {
                result.map_err(mezzenger::Error::Other)
            } else {
                Err(mezzenger::Error::Closed)
            }
        } else {
            Err(mezzenger::Error::Closed)
        }
    }

    #[allow(clippy::type_complexity)]
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<
        Option<
            Result<
                (Incoming, SocketAddr),
                Error<<Codec as Encode>::Error, <Codec as Decode>::Error>,
            >,
        >,
    > {
        if let Some(udp_socket) = &self.udp_socket {
            let mut buf = ReadBuf::new(&mut self.receive_buffer);
            match udp_socket.borrow().poll_recv_from(cx, &mut buf) {
                Poll::Ready(result) => match result {
                    Ok(address) => {
                        let result: Result<Incoming, _> = self.codec.decode(buf.filled());
                        match result {
                            Ok(message) => Poll::Ready(Some(Ok((message, address)))),
                            Err(error) => {
                                Poll::Ready(Some(Err(Error::DeserializationError(error))))
                            }
                        }
                    }
                    Err(error) => match error.kind() {
                        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => {
                            self.udp_socket = None;
                            Poll::Ready(None)
                        }
                        _ => Poll::Ready(Some(Err(Error::IoError(error)))),
                    },
                },
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(None)
        }
    }
}

impl<U, Codec, Incoming, Outgoing> Sink<Outgoing> for Transport<U, Codec, Incoming, Outgoing>
where
    U: Borrow<UdpSocket>,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Error = mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        if self.udp_socket.is_some() {
            self.send_queue.push_back(item);
            Ok(())
        } else {
            Err(mezzenger::Error::Closed)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        if let Some(udp_socket) = &me.udp_socket {
            loop {
                if me.send_buffer.is_empty() && *me.messages_to_send == 0 {
                    if let Some(message) = me.send_queue.pop_front() {
                        let result = me.codec.encode(&mut *me.send_buffer, &message);
                        if let Err(error) = result {
                            me.send_buffer.clear();
                            return Poll::Ready(Err(mezzenger::Error::Other(
                                Error::SerializationError(error),
                            )));
                        } else {
                            *me.messages_to_send += 1;
                        }
                    } else {
                        return Poll::Ready(Ok(()));
                    }
                } else {
                    let bytes_to_send = me.send_buffer.len();
                    let result = udp_socket.borrow().poll_send(cx, me.send_buffer);
                    match result {
                        Poll::Ready(result) => {
                            *me.messages_to_send -= 1;
                            me.send_buffer.clear();
                            match result {
                                Ok(bytes_written) => {
                                    if bytes_written != bytes_to_send {
                                        return Poll::Ready(Err(mezzenger::Error::Other(
                                            Error::SendingError,
                                        )));
                                    }
                                }
                                Err(error) => match error.kind() {
                                    ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => {
                                        *me.udp_socket = None;
                                        return Poll::Ready(Err(mezzenger::Error::Closed));
                                    }
                                    _ => {
                                        return Poll::Ready(Err(mezzenger::Error::Other(
                                            Error::IoError(error),
                                        )))
                                    }
                                },
                            }
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
            }
        } else {
            Poll::Ready(Err(mezzenger::Error::Closed))
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.poll_flush_unpin(cx) {
            Poll::Ready(_) => {
                self.udp_socket = None;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<U, Codec, Incoming, Outgoing> Stream for Transport<U, Codec, Incoming, Outgoing>
where
    U: Borrow<UdpSocket>,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Item = Result<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.poll_recv_from(cx) {
            Poll::Ready(result) => {
                let result = result.map(|result| result.map(|(incoming, _)| incoming));
                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<U, Codec, Incoming, Outgoing> FusedStream for Transport<U, Codec, Incoming, Outgoing>
where
    U: Borrow<UdpSocket>,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    fn is_terminated(&self) -> bool {
        self.udp_socket.is_none()
    }
}

#[cfg(test)]
mod tests {
    use futures::SinkExt;
    use kodec::binary::Codec;
    use mezzenger::Receive;
    use tokio::net::UdpSocket;

    use crate::Transport;

    #[tokio::test]
    async fn test_transport() {
        let left = UdpSocket::bind("127.0.0.1:8080").await.unwrap();
        let right = UdpSocket::bind("127.0.0.1:8081").await.unwrap();

        left.connect(right.local_addr().unwrap()).await.unwrap();
        right.connect(left.local_addr().unwrap()).await.unwrap();

        let mut left: Transport<UdpSocket, Codec, u32, String> =
            Transport::new(left, Codec::default());
        let mut right: Transport<UdpSocket, Codec, String, u32> =
            Transport::new(right, Codec::default());

        left.send("Hello World!".to_string()).await.unwrap();
        left.send("Hello World again!".to_string()).await.unwrap();
        right.send(128).await.unwrap();
        right.send(1).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), "Hello World!");
        assert_eq!(right.receive().await.unwrap(), "Hello World again!");
        assert_eq!(left.receive().await.unwrap(), 128);
        assert_eq!(left.receive().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_unit_message() {
        let left = UdpSocket::bind("127.0.0.1:8082").await.unwrap();
        let right = UdpSocket::bind("127.0.0.1:8083").await.unwrap();

        left.connect(right.local_addr().unwrap()).await.unwrap();
        right.connect(left.local_addr().unwrap()).await.unwrap();

        let mut left: Transport<UdpSocket, Codec, (), ()> = Transport::new(left, Codec::default());
        let mut right: Transport<UdpSocket, Codec, (), ()> =
            Transport::new(right, Codec::default());

        left.send(()).await.unwrap();
        left.send(()).await.unwrap();
        right.send(()).await.unwrap();
        right.send(()).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), ());
        assert_eq!(right.receive().await.unwrap(), ());
        assert_eq!(left.receive().await.unwrap(), ());
        assert_eq!(left.receive().await.unwrap(), ());
    }
}
