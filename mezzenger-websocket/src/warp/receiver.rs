use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Stream, StreamExt};
use kodec::Decode;
use pin_project::pin_project;
use warp::ws::Message;

#[derive(Debug)]
pub enum Error<DeserializationError> {
    DeserializationError(DeserializationError),
    WarpError(warp::Error),
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
            Error::WarpError(error) => write!(f, "warp error occurred: {error}"),
        }
    }
}

impl<DeserializationError> std::error::Error for Error<DeserializationError> where
    DeserializationError: Debug + Display
{
}

/// Message receiver.
///
/// Wraps around stream of [warp::filters::ws::Message] (wrapped in `Result`).
///
/// **NOTE**: This receiver ignores all non-binary (text, ping, pong, close) messages.
#[pin_project]
pub struct Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, warp::Error>>,
    Codec: kodec::Decode,
{
    #[pin]
    inner: S,
    codec: Codec,
    _incoming: PhantomData<Incoming>,
}

impl<S, Codec, Incoming> Stream for Receiver<S, Codec, Incoming>
where
    S: Stream<Item = Result<Message, warp::Error>> + Unpin,
    Codec: kodec::Decode,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    type Item = Result<Incoming, Error<<Codec as Decode>::Error>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(item) => {
                if let Some(item) = item {
                    match item {
                        Ok(message) => {
                            if message.is_binary() {
                                let bytes = message.as_bytes();
                                let result: Result<Incoming, _> = self.codec.decode(bytes);
                                match result {
                                    Ok(message) => Poll::Ready(Some(Ok(message))),
                                    Err(error) => {
                                        Poll::Ready(Some(Err(Error::DeserializationError(error))))
                                    }
                                }
                            } else {
                                Poll::Pending
                            }
                        }
                        Err(error) => Poll::Ready(Some(Err(Error::WarpError(error)))),
                    }
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, Codec, Outgoing> mezzenger::Reliable for Receiver<S, Codec, Outgoing>
where
    S: Stream<Item = Result<Message, warp::Error>>,
    Codec: kodec::Decode,
{
}

impl<S, Codec, Outgoing> mezzenger::Order for Receiver<S, Codec, Outgoing>
where
    S: Stream<Item = Result<Message, warp::Error>>,
    Codec: kodec::Decode,
{
}
