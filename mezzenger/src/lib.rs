//! Message passing infrastructure.

use std::{
    collections::VecDeque,
    fmt::{Debug, Display},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;

/// Transport error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<Other> {
    /// Occurs when transport is closed.
    Closed,
    /// Other non-predefined transport-specific error.
    Other(Other),
}

impl<Other> Display for Error<Other>
where
    Other: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "transport closed"),
            Self::Other(other) => write!(f, "{other}"),
        }
    }
}

impl<Other> std::error::Error for Error<Other> where Other: std::error::Error {}

/// Trait for message senders.
pub trait Send<Message> {
    type Error;
    type Output<'a>: Future<Output = Result<(), Error<Self::Error>>>
    where
        Self: 'a;

    /// Send message.
    #[must_use]
    fn send<'a>(&'a self, message: &Message) -> Self::Output<'a>;

    /// Sink for messages.
    fn sink(&self) -> Sink<Self, Message>
    where
        Self: Sized,
    {
        Sink {
            sender: self,
            buffer: VecDeque::new(),
        }
    }
}

/// Sink for messages.
pub struct Sink<'a, Sender, Message>
where
    Sender: Send<Message>,
{
    sender: &'a Sender,
    buffer: VecDeque<Message>,
}

impl<'a, Sender, Message> Unpin for Sink<'a, Sender, Message> where Sender: Send<Message> {}

impl<'a, Sender, Message> futures::Sink<Message> for Sink<'a, Sender, Message>
where
    Message: 'a,
    Sender: Send<Message>,
{
    type Error = Error<Sender::Error>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.buffer.push_back(item);
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Some(message) = self.buffer.pop_front() {
            Box::pin(self.sender.send(&message)).poll_unpin(cx)
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Trait for message receivers.
pub trait Receive<Message> {
    type Error;
    type Output<'a>: Future<Output = Result<Message, Error<Self::Error>>>
    where
        Self: 'a;

    /// Receive message.
    #[must_use]
    fn receive(&self) -> Self::Output<'_>;

    /// Stream of messages.
    ///
    /// It ends when transport is closed.
    ///
    /// **NOTE**: this stream ignores transport errors other than those signifying
    /// that transport was closed (which are turned into stream termination).
    fn stream(&self) -> Stream<Self, Message>
    where
        Self: Sized,
    {
        Stream {
            receiver: self,
            terminated: false,
            _phantom: PhantomData,
        }
    }
}

/// Stream of messages.
pub struct Stream<'a, Receiver, Message>
where
    Receiver: Receive<Message>,
{
    receiver: &'a Receiver,
    terminated: bool,
    _phantom: PhantomData<Message>,
}

impl<'a, Receiver, Message> Unpin for Stream<'a, Receiver, Message> where Receiver: Receive<Message> {}

impl<'a, Receiver, Message> futures::Stream for Stream<'a, Receiver, Message>
where
    Message: 'a,
    Receiver: Receive<Message>,
{
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Box::pin(self.receiver.receive()).poll_unpin(cx) {
            Poll::Ready(result) => match result {
                Ok(message) => Poll::Ready(Some(message)),
                Err(error) => match error {
                    Error::Closed => {
                        self.terminated = true;
                        Poll::Ready(None)
                    }
                    Error::Other(_) => Poll::Pending,
                },
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<'a, Receiver, Message> futures::stream::FusedStream for Stream<'a, Receiver, Message>
where
    Message: 'a,
    Receiver: Receive<Message>,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

/// Transport that can be closed.
pub trait Close {
    type Output<'a>: Future<Output = ()>
    where
        Self: 'a;

    /// Close transport.
    #[must_use]
    fn close(&self) -> Self::Output<'_>;

    /// Returns `true` if transport is closed (no longer can send and/or receive messages).
    fn is_closed(&self) -> bool;
}

/// Transport guaranteeing that all messages sent are delivered to the receiver
/// (as long as underlying connection is up and running) - i.e. it does not allow
/// messages to be 'lost' in transport.
///
/// **NOTE**: This trait alone does not guarantee that messages will be received
/// in the same order they were sent, neither does it guarantee deduplication
/// - for that transport needs to be marked with [`Order`] trait as well.
///
/// **NOTE to transport implementors**: this trait is only a marker - it's your
/// responsibility to ensure that transport implementation marked with this trait
/// meets guarantees mentioned above.  
pub trait Reliable {}

/// Transport guaranteeing that all messages will arrive in the order they were sent.
///
/// Also guarantees deduplication (the same message won't be received twice).
///
/// **NOTE**: This trait alone does not guarantee that all messages will be received -
/// for that transport needs to be marked with [`Reliable`] trait as well.
///
/// **NOTE to transport implementors**: this trait is only a marker - it's your
/// responsibility to ensure that transport implementation marked with this trait
/// meets guarantees mentioned above.  
pub trait Order {}
