//! Transport for communication over [tokio](https://tokio.rs/)
//! TCP implementation.
//!
//! See [repository](https://github.com/zduny/mezzenger) for more info.
//!
//! ## Example
//!
//! ```ignore
//! let tcp_stream = TcpStream::connect("127.0.0.1:8080").await?;
//!
//! use kodec::binary::Codec;
//! let mut transport: Transport<_, Codec, i32, String> =
//!     Transport::new(tcp_stream, Codec::default());
//!
//! use mezzenger::Receive;
//! let integer = transport.receive().await?;
//!
//! transport.send("Hello World!".to_string()).await?;
//! ```

use std::{
    fmt::{Debug, Display},
    io::ErrorKind,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{Buf, BufMut, BytesMut};
use futures::{ready, stream::FusedStream, Sink, Stream};
use kodec::{Decode, Encode};
use pin_project::pin_project;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::io::{poll_read_buf, poll_write_buf};

pub const DEFAULT_MAX_MESSAGE_SIZE: u32 = 65536;

#[derive(Debug)]
pub enum Error<SerializationError, DeserializationError> {
    MessageTooLarge,
    SerializationError(SerializationError),
    DeserializationError(DeserializationError),
    IoError(std::io::Error),
}

impl<SerializationError, DeserializationError> Display
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Display,
    DeserializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MessageTooLarge => write!(f, "message was too large"),
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

struct ReceiveState {
    pub buffer: BytesMut,
    pub message_size: u32,
    pub receiving_size: bool,
    pub bytes_to_receive: i64,
    pub bytes_to_skip: u32,
}

impl ReceiveState {
    fn new() -> Self {
        ReceiveState {
            buffer: BytesMut::new(),
            message_size: 0,
            receiving_size: true,
            bytes_to_receive: 4,
            bytes_to_skip: 0,
        }
    }
}

/// Transport for communication over [tokio](https://tokio.rs/)'s TCP implementation.
///
/// Wraps over struct implementing [tokio::io::AsyncWrite] and [tokio::io::AsyncRead].
#[pin_project]
pub struct Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    #[pin]
    inner: T,
    send_buffer: BytesMut,
    receive_state: ReceiveState,
    codec: Codec,
    terminated: bool,
    max_message_size: u32,
    _incoming: PhantomData<Incoming>,
    _outgoing: PhantomData<Outgoing>,
}

impl<T, Codec, Incoming, Outgoing> Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    /// Create new transport wrapping a provided struct implementing
    /// [tokio::io::AsyncWrite] and [tokio::io::AsyncRead].
    ///
    /// **NOTE**: By default serialized message size is limited to [DEFAULT_MAX_MESSAGE_SIZE].<br>
    /// Sending or receiving messages of larger size will result in [Error::MessageTooLarge].
    pub fn new(transport: T, codec: Codec) -> Self {
        Transport::new_with_max_message_size(transport, codec, DEFAULT_MAX_MESSAGE_SIZE)
    }

    /// Create new transport wrapping a provided struct implementing
    /// [tokio::io::AsyncWrite] and [tokio::io::AsyncRead].
    ///
    /// Serialized message size will be limited to `max_message_size`.<br>
    /// Sending or receiving messages of larger size will result in [Error::MessageTooLarge].
    pub fn new_with_max_message_size(transport: T, codec: Codec, max_message_size: u32) -> Self {
        Transport {
            inner: transport,
            codec,
            send_buffer: BytesMut::new(),
            receive_state: ReceiveState::new(),
            terminated: false,
            max_message_size,
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }
}

impl<T, Codec, Incoming, Outgoing> Sink<Outgoing> for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Error = mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        if self.terminated {
            Err(mezzenger::Error::Closed)
        } else {
            let me = self.project();
            let size_position = me.send_buffer.len();
            me.send_buffer.put_u32(0);
            let current_length = me.send_buffer.len();
            me.codec
                .encode(me.send_buffer.writer(), &item)
                .map_err(Error::SerializationError)
                .map_err(mezzenger::Error::Other)?;
            let message_size = me.send_buffer.len() - current_length;
            if message_size > *me.max_message_size as usize {
                me.send_buffer.truncate(size_position);
                Err(mezzenger::Error::Other(Error::MessageTooLarge))
            } else {
                let size_slice = &mut me.send_buffer[size_position..(size_position + 4)];
                let message_size = message_size as u32;
                size_slice.swap_with_slice(&mut message_size.to_be_bytes());
                Ok(())
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut me = self.project();

        let result = if me.send_buffer.is_empty() {
            ready!(me.inner.poll_flush(cx))
        } else {
            ready!(poll_write_buf(me.inner.as_mut(), cx, me.send_buffer)).map(|_| ())
        }
        .map_err(|error| match error.kind() {
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => mezzenger::Error::Closed,
            _ => mezzenger::Error::Other(Error::IoError(error)),
        });

        Poll::Ready(result)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        let result = ready!(me.inner.poll_shutdown(cx)).map_err(|error| match error.kind() {
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => mezzenger::Error::Closed,
            _ => mezzenger::Error::Other(Error::IoError(error)),
        });
        Poll::Ready(result)
    }
}

impl<T, Codec, Incoming, Outgoing> Stream for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Item = Result<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }

        let mut me = self.project();
        loop {
            if me.receive_state.bytes_to_receive <= 0 {
                if me.receive_state.receiving_size {
                    let message_size = me.receive_state.buffer.get_u32();
                    me.receive_state.message_size = message_size;
                    me.receive_state.bytes_to_receive += message_size as i64;
                    if message_size > *me.max_message_size {
                        me.receive_state.bytes_to_receive += 4;
                        if me.receive_state.bytes_to_receive > 0 {
                            me.receive_state.bytes_to_skip = message_size;
                        } else {
                            me.receive_state.buffer.advance(message_size as usize);
                        }
                        return Poll::Ready(Some(Err(Error::MessageTooLarge)));
                    } else {
                        me.receive_state.receiving_size = false;
                    }
                } else {
                    let message_size = me.receive_state.message_size as usize;
                    let message = &me.receive_state.buffer[0..message_size];
                    let result: Result<Incoming, _> = me.codec.decode(message);
                    me.receive_state.buffer.advance(message_size);
                    me.receive_state.receiving_size = true;
                    me.receive_state.bytes_to_receive += 4;
                    return {
                        match result {
                            Ok(message) => Poll::Ready(Some(Ok(message))),
                            Err(error) => {
                                Poll::Ready(Some(Err(Error::DeserializationError(error))))
                            }
                        }
                    };
                }
            } else {
                let result = ready!(poll_read_buf(
                    me.inner.as_mut(),
                    cx,
                    &mut me.receive_state.buffer
                ));
                match result {
                    Ok(bytes_read) => {
                        if bytes_read == 0 {
                            *me.terminated = true;
                            return Poll::Ready(None);
                        }
                        me.receive_state.bytes_to_receive = me
                            .receive_state
                            .bytes_to_receive
                            .saturating_sub_unsigned(bytes_read as u64);
                        if me.receive_state.bytes_to_skip > 0 {
                            let buffer_len = me.receive_state.buffer.len();
                            let skipped = buffer_len.min(me.receive_state.bytes_to_skip as usize);
                            if skipped == buffer_len {
                                me.receive_state.buffer.clear();
                            } else {
                                me.receive_state.buffer.advance(skipped);
                            }
                            me.receive_state.bytes_to_skip = me
                                .receive_state
                                .bytes_to_skip
                                .saturating_sub(skipped as u32);
                        }
                    }
                    Err(error) => match error.kind() {
                        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => {
                            *me.terminated = true;
                            return Poll::Ready(None);
                        }
                        _ => return Poll::Ready(Some(Err(Error::IoError(error)))),
                    },
                }
            }
        }
    }
}

impl<T, Codec, Incoming, Outgoing> FusedStream for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<T, Codec, Incoming, Outgoing> mezzenger::Reliable for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
}

impl<T, Codec, Incoming, Outgoing> mezzenger::Order for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsyncWrite + AsyncRead,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
}

#[cfg(test)]
mod tests {
    use futures::{stream, SinkExt, StreamExt};
    use kodec::binary::Codec;
    use mezzenger::{Messages, Receive};
    use tokio::net::{TcpListener, TcpStream};

    use crate::{Error, Transport};

    #[tokio::test]
    async fn test_transport() {
        let left = TcpListener::bind("127.0.0.1:8080").await.unwrap();
        let right = TcpStream::connect("127.0.0.1:8080").await.unwrap();

        let (left, _) = left.accept().await.unwrap();

        let mut left: Transport<TcpStream, Codec, u32, String> =
            Transport::new(left, Codec::default());
        let mut right: Transport<TcpStream, Codec, String, u32> =
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
        let left = TcpListener::bind("127.0.0.1:8081").await.unwrap();
        let right = TcpStream::connect("127.0.0.1:8081").await.unwrap();

        let (left, _) = left.accept().await.unwrap();

        let mut left: Transport<TcpStream, Codec, (), ()> = Transport::new(left, Codec::default());
        let mut right: Transport<TcpStream, Codec, (), ()> =
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

    #[tokio::test]
    async fn test_stream() {
        let left = TcpListener::bind("127.0.0.1:8082").await.unwrap();
        let right = TcpStream::connect("127.0.0.1:8082").await.unwrap();

        let (left, _) = left.accept().await.unwrap();

        let mut left: Transport<TcpStream, Codec, (), u32> = Transport::new(left, Codec::default());
        let right: Transport<TcpStream, Codec, u32, ()> = Transport::new(right, Codec::default());

        left.send_all(&mut stream::iter(vec![1, 2, 3].into_iter().map(Ok)))
            .await
            .unwrap();
        drop(left);

        assert_eq!(right.messages().collect::<Vec<u32>>().await, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_size_limit() {
        let left = TcpListener::bind("127.0.0.1:8084").await.unwrap();
        let right = TcpStream::connect("127.0.0.1:8084").await.unwrap();

        let (left, _) = left.accept().await.unwrap();

        let mut left: Transport<TcpStream, Codec, String, String> =
            Transport::new_with_max_message_size(left, Codec::default(), 15);
        let mut right: Transport<TcpStream, Codec, String, String> =
            Transport::new(right, Codec::default());

        left.send("Hey".to_string()).await.unwrap();
        assert!(matches!(
            left.send("Hello, hello, hello".to_string()).await,
            Err(mezzenger::Error::Other(Error::MessageTooLarge))
        ));
        left.send("Hi".to_string()).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), "Hey");
        assert_eq!(right.receive().await.unwrap(), "Hi");

        right.send("Hey".to_string()).await.unwrap();
        for _i in 0..139 {
            right.send("Hello, hello, hello".to_string()).await.unwrap();
        }
        right.send("Hi".to_string()).await.unwrap();

        assert_eq!(left.receive().await.unwrap(), "Hey");
        for _i in 0..139 {
            assert!(matches!(
                left.receive().await,
                Err(mezzenger::Error::Other(Error::MessageTooLarge))
            ));
        }
        assert_eq!(left.receive().await.unwrap(), "Hi");

        right.send("Hey".to_string()).await.unwrap();
        for _i in 0..17 {
            right.send("Hello, hello, hello".to_string()).await.unwrap();
            right
                .send("Hello, hello, hello, hi".to_string())
                .await
                .unwrap();
        }
        right.send("Hi".to_string()).await.unwrap();

        assert_eq!(left.receive().await.unwrap(), "Hey");
        for _i in 0..17 {
            assert!(matches!(
                left.receive().await,
                Err(mezzenger::Error::Other(Error::MessageTooLarge))
            ));
            assert!(matches!(
                left.receive().await,
                Err(mezzenger::Error::Other(Error::MessageTooLarge))
            ));
        }
        assert_eq!(left.receive().await.unwrap(), "Hi");
    }
}
