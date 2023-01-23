//! Wrapper transport turning a [numbered] (but not necessarily [ordered]) transport 
//! into an [ordered] transport, discarding old messages (polling a transport for 
//! the next message will return the latest received message, ignoring messages 
//! received before).
//! 
//! Potentially useful when user doesn't care about stale messages 
//! (for example - multiplayer video games).
//! 
//! [numbered]: crate::numbered::Number
//! [ordered]: mezzenger::Order

use std::{
    marker::PhantomData,
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures::{stream::FusedStream, Sink, Stream, StreamExt};
use pin_project::pin_project;

use crate::numbered::{Number, Unwrap};

/// Wrapper transport turning a [numbered] (but not necessarily [ordered]) transport 
/// into an [ordered] transport, discarding old messages (polling a transport for 
/// the next message will return the latest received message, ignoring messages 
/// received before).
/// 
/// [numbered]: crate::numbered::Number
/// [ordered]: mezzenger::Order
#[pin_project]
pub struct LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E>,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
    #[pin]
    inner: T,
    last_number: Option<N>,
    _error: PhantomData<E>,
    _incoming: PhantomData<Incoming>,
    _outgoing: PhantomData<Outgoing>,
}

impl<T, E, N, Incoming, Outgoing> LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E>,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
    /// Wrap a provided [numbered] transport turning it into [ordered] transport returning latest 
    /// message when polling it for the next received message.
    ///
    /// [numbered]: crate::numbered::Number
    /// [ordered]: mezzenger::Order
    pub fn new(transport: T) -> Self {
        LatestOnly {
            inner: transport,
            last_number: None,
            _error: PhantomData,
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }

    /// Return an [unwrapping] transport.
    /// 
    /// By default [`LatestOnly`] stream returns messages in their original form 
    /// (with number attached).<br>
    /// If `Incoming` message implements [Unwrap] you can transform this transport into
    /// [`LatestOnlyUnwrapping`] automatically unwrapping messages, useful when 
    /// you don't care about the message number.
    /// 
    /// [Unwrap]: crate::numbered::Unwrap 
    /// [unwrapping]: crate::numbered::Unwrap 
    pub fn into_unwrapping<I>(self) -> LatestOnlyUnwrapping<T, E, N, Incoming, I, Outgoing>
    where
        Incoming: Unwrap<Output = I>,
    {
        LatestOnlyUnwrapping { inner: self }
    }
}

impl<T, E, N, Incoming, Outgoing> Sink<Outgoing> for LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E>,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
    type Error = mezzenger::Error<E>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        let me = self.project();
        me.inner.start_send(item)
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

impl<T, E, N, Incoming, Outgoing> Stream for LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E>,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
    type Item = Result<Incoming, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut me = self.project();
        let mut latest = None;
        loop {
            match me.inner.poll_next_unpin(cx) {
                Poll::Ready(item) => {
                    if let Some(item) = item {
                        let item = item?;
                        let number = item.number();
                        if me.last_number.is_none() || &number > me.last_number.as_ref().unwrap() {
                            latest = Some(item);
                            *me.last_number = Some(number);
                        }
                    } else {
                        return Poll::Ready(None);
                    }
                }
                Poll::Pending => {
                    return if let Some(latest) = latest {
                        Poll::Ready(Some(Ok(latest)))
                    } else {
                        Poll::Pending
                    }
                }
            }
        }
    }
}

impl<T, E, N, Incoming, Outgoing> FusedStream for LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E> + FusedStream,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<T, E, N, Incoming, Outgoing> mezzenger::Order for LatestOnly<T, E, N, Incoming, Outgoing>
where
    T: mezzenger::Transport<Incoming, Outgoing, E>,
    Incoming: Number<Output = N>,
    for<'a> &'a N: PartialOrd,
{
}

/// Transport returned by [`LatestOnly::into_unwrapping`] method.
#[pin_project]
pub struct LatestOnlyUnwrapping<T, E, N, I, Incoming, Outgoing>
where
    T: mezzenger::Transport<I, Outgoing, E>,
    I: Number<Output = N> + Unwrap<Output = Incoming>,
    for<'a> &'a N: PartialOrd,
{
    #[pin]
    inner: LatestOnly<T, E, N, I, Outgoing>,
}

impl<T, E, N, I, Incoming, Outgoing> Sink<Outgoing>
    for LatestOnlyUnwrapping<T, E, N, I, Incoming, Outgoing>
where
    T: mezzenger::Transport<I, Outgoing, E>,
    I: Number<Output = N> + Unwrap<Output = Incoming>,
    for<'a> &'a N: PartialOrd,
{
    type Error = mezzenger::Error<E>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let me = self.project();
        me.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Outgoing) -> Result<(), Self::Error> {
        let me = self.project();
        me.inner.start_send(item)
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

impl<T, E, N, I, Incoming, Outgoing> Stream for LatestOnlyUnwrapping<T, E, N, I, Incoming, Outgoing>
where
    T: mezzenger::Transport<I, Outgoing, E>,
    I: Number<Output = N> + Unwrap<Output = Incoming>,
    for<'a> &'a N: PartialOrd,
{
    type Item = Result<Incoming, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();
        let result = ready!(me.inner.poll_next(cx));
        Poll::Ready(result.map(|result| result.map(|result| result.unwrap())))
    }
}

impl<T, E, N, I, Incoming, Outgoing> FusedStream
    for LatestOnlyUnwrapping<T, E, N, I, Incoming, Outgoing>
where
    T: mezzenger::Transport<I, Outgoing, E> + FusedStream,
    I: Number<Output = N> + Unwrap<Output = Incoming>,
    for<'a> &'a N: PartialOrd,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<T, E, N, I, Incoming, Outgoing> mezzenger::Order
    for LatestOnlyUnwrapping<T, E, N, I, Incoming, Outgoing>
where
    T: mezzenger::Transport<I, Outgoing, E>,
    I: Number<Output = N> + Unwrap<Output = Incoming>,
    for<'a> &'a N: PartialOrd,
{
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

    use crate::{
        numbered::{Numbered, Unwrap, Wrapper},
        LatestOnly,
    };

    async fn test_transport_inner() {
        let (left, right) = transports();

        let left = Numbered::new_usize(left);
        let right = Numbered::new_usize(right);

        let mut left = LatestOnly::new(left);
        let mut right = LatestOnly::new(right);

        left.send(1).await.unwrap();
        left.send(2).await.unwrap();
        left.send(3).await.unwrap();

        assert_eq!(right.receive().await.unwrap().unwrap(), 3);

        right.send(1).await.unwrap();
        right.send(2).await.unwrap();
        right.send(3).await.unwrap();

        assert_eq!(left.receive().await.unwrap().unwrap(), 3);
    }

    async fn test_order_inner() {
        let (mut left, right) = transports();

        let right = Numbered::new_usize(right);

        let mut right = LatestOnly::new(right);

        left.send(Wrapper {
            number: 1,
            wrapped: 2,
        })
        .await
        .unwrap();
        left.send(Wrapper {
            number: 2,
            wrapped: 3,
        })
        .await
        .unwrap();
        left.send(Wrapper {
            number: 0,
            wrapped: 1,
        })
        .await
        .unwrap();

        right.send(1).await.unwrap();

        assert_eq!(right.receive().await.unwrap().unwrap(), 3);
        assert_eq!(left.receive().await.unwrap().unwrap(), 1);
    }

    async fn test_unwrapping_inner() {
        let (left, right) = transports();

        let left = Numbered::new_usize(left);
        let right = Numbered::new_usize(right);

        let mut left = LatestOnly::new(left).into_unwrapping();
        let mut right = LatestOnly::new(right).into_unwrapping();

        left.send(1).await.unwrap();
        left.send(2).await.unwrap();
        left.send(3).await.unwrap();

        assert_eq!(right.receive().await.unwrap(), 3);

        right.send(1).await.unwrap();
        right.send(2).await.unwrap();
        right.send(3).await.unwrap();

        assert_eq!(left.receive().await.unwrap(), 3);
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

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_unwrapping() {
        test_unwrapping_inner().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_unwrapping() {
        test_unwrapping_inner().await
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_order() {
        test_order_inner().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_order() {
        test_order_inner().await
    }
}
