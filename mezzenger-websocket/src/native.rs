//! Transport for communication over
//! [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
//! while using [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite).

use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{stream::FusedStream, Sink, SinkExt, Stream, StreamExt};
use kodec::{Decode, Encode};
use pin_project::pin_project;
use serde::Serialize;
use tungstenite::Message;

#[derive(Debug)]
pub enum Error<SerializationError, DeserializationError> {
    SerializationError(SerializationError),
    DeserializationError(DeserializationError),
    TungsteniteError(tungstenite::Error),
}

impl<SerializationError, DeserializationError> Display
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Display,
    DeserializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SerializationError(error) => write!(f, "failed to serialize message: {error}"),
            Error::DeserializationError(error) => {
                write!(f, "failed to deserialize message: {error}")
            }
            Error::TungsteniteError(error) => write!(f, "tungstenite error occurred: {error}"),
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

/// Web Socket transport for [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite).
///
/// Wraps around [tokio_tungstenite::WebSocketStream].
///
/// **NOTE**: This transport's receiving stream ignores all non-binary (text, ping, pong, close) messages.
#[pin_project]
pub struct Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    #[pin]
    inner: T,
    codec: Codec,
    terminated: bool,
    _incoming: PhantomData<Incoming>,
    _outgoing: PhantomData<Outgoing>,
}

impl<T, Codec, Incoming, Outgoing> Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    /// Create new transport wrapping a provided `[tokio_tungstenite::WebSocketStream]`.
    pub fn new(stream: T, codec: Codec) -> Self {
        Transport {
            inner: stream,
            codec,
            terminated: false,
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }
}

impl<T, Codec, Incoming, Outgoing> Sink<Outgoing> for Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Error = mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx).map_err(map_error)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        let mut buffer = vec![];
        self.codec
            .encode(&mut buffer, &item)
            .map_err(Error::SerializationError)
            .map_err(mezzenger::Error::Other)?;
        let message = Message::binary(buffer);
        self.inner.start_send_unpin(message).map_err(map_error)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx).map_err(map_error)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx).map_err(map_error)
    }
}

fn map_error<SerializationError, DeserializationError>(
    tungstenite_error: tungstenite::Error,
) -> mezzenger::Error<Error<SerializationError, DeserializationError>> {
    if matches!(
        tungstenite_error,
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed
    ) {
        mezzenger::Error::Closed
    } else {
        mezzenger::Error::Other(self::Error::TungsteniteError(tungstenite_error))
    }
}

impl<T, Codec, Incoming, Outgoing> Stream for Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
    type Item = Result<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(item) => {
                if let Some(item) = item {
                    match item {
                        Ok(message) => match message {
                            Message::Binary(message) => {
                                let result: Result<Incoming, _> = self.codec.decode(&message[..]);
                                match result {
                                    Ok(message) => Poll::Ready(Some(Ok(message))),
                                    Err(error) => {
                                        Poll::Ready(Some(Err(Error::DeserializationError(error))))
                                    }
                                }
                            }
                            Message::Close(_) => {
                                self.terminated = true;
                                Poll::Ready(None)
                            }
                            _ => Poll::Pending,
                        },
                        Err(error) => match &error {
                            tungstenite::Error::Protocol(
                                tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                            ) => {
                                self.terminated = true;
                                Poll::Ready(None)
                            }
                            _ => Poll::Ready(Some(Err(Error::TungsteniteError(error)))),
                        },
                    }
                } else {
                    self.terminated = true;
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T, Codec, Incoming, Outgoing> FusedStream for Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
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
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
}

impl<T, Codec, Incoming, Outgoing> mezzenger::Order for Transport<T, Codec, Incoming, Outgoing>
where
    T: Sink<Message, Error = tungstenite::Error>
        + Stream<Item = Result<Message, tungstenite::Error>>
        + Unpin,
    Codec: kodec::Codec,
    for<'de> Incoming: serde::de::Deserialize<'de>,
    Outgoing: Serialize,
{
}
