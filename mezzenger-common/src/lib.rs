use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{future::FusedFuture, Future};

pub mod rc;

#[cfg(not(target_arch = "wasm32"))]
pub mod sync;

#[cfg(feature = "browser")]
pub mod browser;

pub trait Sender<Transport, Message, Error> {
    fn send(transport: &Transport, message: Message) -> Result<(), mezzenger::Error<Error>>;
}

/// Future returned by [send] method.
///
/// [send]: mezzenger::Send::send
pub struct Send<'a, Transport, Message, Error, S>
where
    S: Sender<Transport, Message, Error>,
{
    transport: &'a Transport,
    message: Option<Message>,
    _sender: PhantomData<S>,
    _error: PhantomData<Error>,
}

impl<'a, Transport, Message, Error, S> Send<'a, Transport, Message, Error, S>
where
    S: Sender<Transport, Message, Error>,
{
    /// Create new future for [send] method.
    ///
    /// [send]: mezzenger::Send::send
    pub fn new(transport: &'a Transport, message: Message) -> Self {
        Send {
            transport,
            message: Some(message),
            _sender: PhantomData,
            _error: PhantomData,
        }
    }
}

impl<'a, Transport, Message, Error, S> Unpin for Send<'a, Transport, Message, Error, S> where
    S: Sender<Transport, Message, Error>
{
}

impl<'a, Transport, Message, Error, S> Future for Send<'a, Transport, Message, Error, S>
where
    S: Sender<Transport, Message, Error>,
{
    type Output = Result<(), mezzenger::Error<Error>>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            Poll::Ready(S::send(self.transport, self.message.take().unwrap()))
        } else {
            Poll::Pending
        }
    }
}

impl<'a, Transport, Message, Error, S> FusedFuture for Send<'a, Transport, Message, Error, S>
where
    S: Sender<Transport, Message, Error>,
{
    fn is_terminated(&self) -> bool {
        self.message.is_none()
    }
}
