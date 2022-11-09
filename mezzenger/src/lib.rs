//! Message passing infrastructure.

use std::{
    fmt::Display,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    future::FusedFuture,
    stream::{FusedStream, Next},
    Future, FutureExt, Stream, StreamExt,
};

use pin_project::pin_project;

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

/// Trait for message receivers.
pub trait Receive<Message, Error> {
    fn receive(&mut self) -> Recv<Self>;
}

impl<T, Message, Error> Receive<Message, Error> for T
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
{
    /// Receive message from transport.
    fn receive(&mut self) -> Recv<Self> {
        let next = self.next();
        Recv {
            next,
            terminated: false,
        }
    }
}

/// Future returned by [receive] method.
///
/// [receive]: mezzenger::Receive::receive
pub struct Recv<'a, T>
where
    T: ?Sized,
{
    next: Next<'a, T>,
    terminated: bool,
}

impl<'a, T, Message, Error> Future for Recv<'a, T>
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
{
    type Output = Result<Message, self::Error<Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.next.poll_unpin(cx) {
            Poll::Ready(item) => {
                self.terminated = true;
                if let Some(item) = item {
                    Poll::Ready(item.map_err(|error| self::Error::Other(error)))
                } else {
                    Poll::Ready(Err(self::Error::Closed))
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<'a, T, Message, Error> FusedFuture for Recv<'a, T>
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

/// Utility trait for creating message stream with filtered out errors.
pub trait Messages<T, Message, Error>
where
    Self: Sized,
{
    /// Message stream with filtered out errors.
    ///
    /// Calls a callback when error occurs.
    fn messages_with_error_callback<F>(self, error_callback: F) -> MessageStream<Self, F>
    where
        F: FnMut(Error);

    /// Message stream with filtered out errors.
    fn messages(self) -> MessageStream<Self, fn(Error) -> ()> {
        self.messages_with_error_callback(|_| {})
    }
}

impl<T, Message, Error> Messages<T, Message, Error> for T
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
{
    fn messages_with_error_callback<F>(self, error_callback: F) -> MessageStream<Self, F>
    where
        F: FnMut(Error),
    {
        MessageStream {
            stream: self,
            error_callback,
            terminated: false,
        }
    }
}

/// Stream of messages.
///
/// Returned by [messages] function.
///
/// [messages]: self::Messages::messages
#[pin_project]
pub struct MessageStream<T, F> {
    #[pin]
    stream: T,
    error_callback: F,
    terminated: bool,
}

impl<T, F, Message, Error> Stream for MessageStream<T, F>
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
    F: FnMut(Error),
{
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.stream.poll_next_unpin(cx) {
            Poll::Ready(item) => {
                if let Some(item) = item {
                    match item {
                        Ok(message) => Poll::Ready(Some(message)),
                        Err(error) => {
                            (self.error_callback)(error);
                            Poll::Pending
                        }
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

impl<T, F, Message, Error> FusedStream for MessageStream<T, F>
where
    T: Stream<Item = Result<Message, Error>> + Unpin,
    F: FnMut(Error),
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
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
