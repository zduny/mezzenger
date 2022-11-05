use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Sink, SinkExt};
use kodec::Encode;
use pin_project::pin_project;
use serde::Serialize;
use warp::ws::Message;

#[derive(Debug)]
pub enum Error<SerializationError> {
    SerializationError(SerializationError),
    WarpError(warp::Error),
}

impl<SerializationError> Display for Error<SerializationError>
where
    SerializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SerializationError(error) => write!(f, "failed to serialize message: {error}"),
            Error::WarpError(error) => write!(f, "warp error occurred: {error}"),
        }
    }
}

impl<SerializationError> std::error::Error for Error<SerializationError> where
    SerializationError: Debug + Display
{
}

/// Message sender.
///
/// Wraps around sink of [warp::filters::ws::Message].
#[pin_project]
pub struct Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = warp::Error>,
    Codec: kodec::Encode,
{
    #[pin]
    inner: S,
    codec: Codec,
    _outgoing: PhantomData<Outgoing>,
}

impl<S, Codec, Outgoing> Sink<&Outgoing> for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = warp::Error> + Unpin,
    Outgoing: Serialize,
    Codec: kodec::Encode,
{
    type Error = mezzenger::Error<Error<<Codec as Encode>::Error>>;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx).map_err(map_warp_error)
    }

    fn start_send(mut self: Pin<&mut Self>, item: &Outgoing) -> Result<(), Self::Error> {
        let mut buffer = vec![];
        self.codec
            .encode(&mut buffer, item)
            .map_err(Error::SerializationError)
            .map_err(mezzenger::Error::Other)?;
        let message = Message::binary(buffer);
        self.inner.start_send_unpin(message).map_err(map_warp_error)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx).map_err(map_warp_error)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx).map_err(map_warp_error)
    }
}

fn map_warp_error<SerializationError>(
    warp_error: warp::Error,
) -> mezzenger::Error<Error<SerializationError>> {
    use std::error::Error;
    if let Some(error) = warp_error.source() {
        if let Some(error) = error.downcast_ref::<tungstenite::Error>() {
            if matches!(
                error,
                tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed
            ) {
                mezzenger::Error::Closed
            } else {
                mezzenger::Error::Other(self::Error::WarpError(warp_error))
            }
        } else {
            mezzenger::Error::Other(self::Error::WarpError(warp_error))
        }
    } else {
        mezzenger::Error::Other(self::Error::WarpError(warp_error))
    }
}

impl<S, Codec, Outgoing> mezzenger::Reliable for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = warp::Error>,
    Codec: kodec::Encode,
{
}

impl<S, Codec, Outgoing> mezzenger::Order for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = warp::Error>,
    Codec: kodec::Encode,
{
}
