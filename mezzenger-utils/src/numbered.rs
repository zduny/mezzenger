//! Wrapper transport attaching a number to messages.

use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{stream::FusedStream, Sink, Stream};
use num::{traits::WrappingAdd, One, Zero};
use pin_project::pin_project;
use serde::{Serialize, Deserialize};

/// Trait implemented by messages with attached number.
pub trait Number {
    /// Message number type.
    type Output;

    /// Message number.
    fn number(&self) -> Self::Output;
}

/// Trait implemented by numbered messages that can be unwrapped into their original form.
pub trait Unwrap {
    /// Type of wrapped message.
    type Output;

    /// Unwrap message.
    fn unwrap(self) -> Self::Output;
}

/// Message wrapper used by [`Numbered`] transport.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wrapper<N, T> {
    /// Message number.
    pub number: N,

    /// Wrapped message.
    pub wrapped: T,
}

impl<N, T> Number for Wrapper<N, T>
where
    N: Clone,
{
    type Output = N;

    fn number(&self) -> Self::Output {
        self.number.clone()
    }
}

impl<N, T> Unwrap for Wrapper<N, T> {
    type Output = T;

    fn unwrap(self) -> Self::Output {
        self.wrapped
    }
}

/// Wrapper transport attaching number to sent messages.
/// 
/// Received messages are of [`Wrapper`] type.
/// 
/// First message number is [zero].<br>
/// Next message number = previous message number + [one].
/// 
/// Message numbers will [wrap] when reaching maximum value.
/// 
/// [zero]: num::Zero
/// [one]: num::One
/// [wrap]: num::traits::WrappingAdd
#[pin_project]
pub struct Numbered<N, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<N, Incoming>, Wrapper<N, Outgoing>, E>,
    N: Clone + Zero + One + WrappingAdd,
{
    #[pin]
    inner: T,
    current_number: N,
    _error: PhantomData<E>,
    _number: PhantomData<N>,
    _incoming: PhantomData<Incoming>,
    _outgoing: PhantomData<Outgoing>,
}

impl<N, T, E, Incoming, Outgoing> Numbered<N, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<N, Incoming>, Wrapper<N, Outgoing>, E>,
    N: Clone + Zero + One + WrappingAdd,
{
    /// Create new [numbered] transport wrapping provided transport.
    /// 
    /// [numbered]: self::Number 
    pub fn new(transport: T) -> Self {
        Numbered {
            inner: transport,
            current_number: N::zero(),
            _error: PhantomData,
            _number: PhantomData,
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }

    /// Number that will be given attached the next sent message.
    pub fn current_number(&self) -> N {
        self.current_number.clone()
    }
}

impl<T, E, Incoming, Outgoing> Numbered<usize, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<usize, Incoming>, Wrapper<usize, Outgoing>, E>,
{
    /// Create new [`Numbered`] transport using [`usize`] type as  message number.
    pub fn new_usize(transport: T) -> Self {
        Numbered::new(transport)
    }
}

impl<T, E, Incoming, Outgoing> Numbered<u32, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<u32, Incoming>, Wrapper<u32, Outgoing>, E>,
{
    /// Create new [`Numbered`] transport using [`u32`] type as  message number.
    pub fn new_u32(transport: T) -> Self {
        Numbered::new(transport)
    }
}

impl<T, E, Incoming, Outgoing> Numbered<u64, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<u64, Incoming>, Wrapper<u64, Outgoing>, E>,
{
    /// Create new [`Numbered`] transport using [`u64`] type as  message number.
    pub fn new_u64(transport: T) -> Self {
        Numbered::new(transport)
    }
}

impl<T, E, Incoming, Outgoing> Numbered<u128, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<u128, Incoming>, Wrapper<u128, Outgoing>, E>,
{
    /// Create new [`Numbered`] transport using [`u128`] type as  message number.
    pub fn new_u128(transport: T) -> Self {
        Numbered::new(transport)
    }
}

impl<N, T, E, Incoming, Outgoing> Sink<Outgoing> for Numbered<N, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<N, Incoming>, Wrapper<N, Outgoing>, E>,
    N: Clone + Zero + One + WrappingAdd,
{
    type Error = mezzenger::Error<E>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        let me = self.project();
        let item = Wrapper {
            number: me.current_number.clone(),
            wrapped: item,
        };
        let result = me.inner.start_send(item);
        if result.is_ok() {
            *me.current_number = me.current_number.wrapping_add(&One::one());
        }
        result
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.inner.poll_close(cx)
    }
}

impl<N, T, E, Incoming, Outgoing> Stream for Numbered<N, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<N, Incoming>, Wrapper<N, Outgoing>, E>,
    N: Clone + Zero + One + WrappingAdd,
{
    type Item = Result<Wrapper<N, Incoming>, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();
        me.inner.poll_next(cx)
    }
}

impl<N, T, E, Incoming, Outgoing> FusedStream for Numbered<N, T, E, Incoming, Outgoing>
where
    T: mezzenger::Transport<Wrapper<N, Incoming>, Wrapper<N, Outgoing>, E> + FusedStream,
    N: Clone + Zero + One + WrappingAdd,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

#[cfg(test)]
mod tests {
    use futures::SinkExt;
    use mezzenger::Receive;
    use mezzenger_channel::transports;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    use crate::numbered::{Numbered, Wrapper};

    async fn test_transport_inner() {
        let (left, right) = transports();

        let mut left = Numbered::new_usize(left);
        let mut right = Numbered::new_usize(right);

        left.send(1).await.unwrap();
        left.send(2).await.unwrap();
        left.send(3).await.unwrap();

        assert_eq!(
            right.receive().await.unwrap(),
            Wrapper {
                number: 0,
                wrapped: 1,
            }
        );
        assert_eq!(
            right.receive().await.unwrap(),
            Wrapper {
                number: 1,
                wrapped: 2,
            }
        );
        assert_eq!(
            right.receive().await.unwrap(),
            Wrapper {
                number: 2,
                wrapped: 3,
            }
        );

        right.send(1).await.unwrap();
        right.send(2).await.unwrap();
        right.send(3).await.unwrap();

        assert_eq!(
            left.receive().await.unwrap(),
            Wrapper {
                number: 0,
                wrapped: 1,
            }
        );
        assert_eq!(
            left.receive().await.unwrap(),
            Wrapper {
                number: 1,
                wrapped: 2,
            }
        );
        assert_eq!(
            left.receive().await.unwrap(),
            Wrapper {
                number: 2,
                wrapped: 3,
            }
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_transport() {
        test_transport_inner().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_transport() {
        test_transport_inner().await
    }
}
