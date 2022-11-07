use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{stream::FusedStream, Stream, StreamExt};
use kodec::Decode;
use pin_project::pin_project;
use tungstenite::Message;

#[derive(Debug)]
pub enum Error<DeserializationError> {
    DeserializationError(DeserializationError),
    TungsteniteError(tungstenite::Error),
}

impl<DeserializationError> Display for Error<DeserializationError>
where
    DeserializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DeserializationError(error) => {
                write!(f, "failed to deserialize message: {error}")
            }
            Error::TungsteniteError(error) => write!(f, "tungstenite error occurred: {error}"),
        }
    }
}

impl<DeserializationError> std::error::Error for Error<DeserializationError> where
    DeserializationError: Debug + Display
{
}

/// Message receiver.
///
/// Wraps around stream of [tungstenite::Message] (wrapped in `Result`).
///
/// **NOTE**: This receiver ignores all non-binary (text, ping, pong, close) messages.
#[pin_project]
pub struct Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, tungstenite::Error>>,
    Codec: kodec::Decode,
{
    #[pin]
    inner: S,
    codec: Codec,
    terminated: bool,
    _incoming: PhantomData<Incoming>,
}

impl<S, Codec, Incoming> Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, tungstenite::Error>>,
    Codec: kodec::Decode,
{
    /// Create new receiver wrapping a provided `stream`.
    pub fn new(stream: S, codec: Codec) -> Self {
        Receiver {
            inner: stream,
            codec,
            terminated: false,
            _incoming: PhantomData,
        }
    }
}

impl<S, Codec, Incoming> Stream for Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
    Codec: kodec::Decode,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    type Item = Result<Incoming, Error<<Codec as Decode>::Error>>;

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

impl<S, Codec, Incoming> FusedStream for Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
    Codec: kodec::Decode,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<S, Codec, Outgoing> mezzenger::Reliable for Receiver<S, Codec, Outgoing>
where
    S: Stream<Item = Result<Message, tungstenite::Error>>,
    Codec: kodec::Decode,
{
}

impl<S, Codec, Outgoing> mezzenger::Order for Receiver<S, Codec, Outgoing>
where
    S: Stream<Item = Result<Message, tungstenite::Error>>,
    Codec: kodec::Decode,
{
}
