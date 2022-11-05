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
use tungstenite::Message;

#[derive(Debug)]
pub enum Error<SerializationError> {
    SerializationError(SerializationError),
    TungsteniteError(tungstenite::Error),
}

impl<SerializationError> Display for Error<SerializationError>
where
    SerializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SerializationError(error) => write!(f, "failed to serialize message: {error}"),
            Error::TungsteniteError(error) => write!(f, "tungstenite error occurred: {error}"),
        }
    }
}

impl<SerializationError> std::error::Error for Error<SerializationError> where
    SerializationError: Debug + Display
{
}

/// Message sender.
///
/// Wraps around sink of [tungstenite::Message;].
#[pin_project]
pub struct Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = tungstenite::Error>,
    Codec: kodec::Encode,
{
    #[pin]
    inner: S,
    codec: Codec,
    _outgoing: PhantomData<Outgoing>,
}

impl<S, Codec, Outgoing> Sink<&Outgoing> for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = tungstenite::Error> + Unpin,
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
    tungstenite_error: tungstenite::Error,
) -> mezzenger::Error<Error<SerializationError>> {
    if matches!(
        tungstenite_error,
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed
    ) {
        mezzenger::Error::Closed
    } else {
        mezzenger::Error::Other(self::Error::TungsteniteError(tungstenite_error))
    }
}

impl<S, Codec, Outgoing> mezzenger::Reliable for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = tungstenite::Error>,
    Codec: kodec::Encode,
{
}

impl<S, Codec, Outgoing> mezzenger::Order for Sender<S, Codec, Outgoing>
where
    S: Sink<Message, Error = tungstenite::Error>,
    Codec: kodec::Encode,
{
}
