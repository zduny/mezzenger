use std::{
    collections::VecDeque,
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll, Waker},
};

use futures::future::FusedFuture;
use parking_lot::Mutex;

/// Transport storing messages in local queue.
///
/// It does not send messages anywhere (to other side of transport) - messages 'received'
/// from this transport are the same messages that were 'sent' into it locally.
///
/// **NOTE**: do NOT use this transport to send data between threads or async tasks,
/// use appropriate channels instead as they will most likely be much more performant.
pub struct Transport<Message> {
    state: Mutex<State<Message>>,
}

impl<Message> Transport<Message> {
    /// Creates new dummy transport.
    pub fn new() -> Self {
        Transport {
            state: Mutex::new(State::new()),
        }
    }

    fn wake_next(&self) {
        while let Some(waker) = self.state.lock().wakers.pop_front() {
            if let Some(waker) = waker.upgrade() {
                let mut waker = waker.lock();
                waker.woken = true;
                waker.waker.wake_by_ref();
                break;
            }
        }
    }

    fn close(&self) {
        self.state.lock().closed = true;
        self.wake_next();
    }
}

impl<Message> Default for Transport<Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message> Drop for Transport<Message> {
    fn drop(&mut self) {
        self.close();
    }
}

struct State<T> {
    buffer: VecDeque<T>,
    wakers: VecDeque<Weak<Mutex<ReceiveWaker>>>,
    closed: bool,
}

impl<T> State<T> {
    fn new() -> Self {
        State {
            buffer: VecDeque::new(),
            wakers: VecDeque::new(),
            closed: false,
        }
    }
}

impl<Message> mezzenger::Reliable for Transport<Message> {}

impl<Message> mezzenger::Order for Transport<Message> {}

impl<Message> mezzenger::Close for Transport<Message> {
    type Output<'a> = Close<'a, Message> where Self: 'a;

    fn close(&self) -> Self::Output<'_> {
        Close {
            transport: self,
            terminated: false,
        }
    }

    fn is_closed(&self) -> bool {
        self.state.lock().closed
    }
}

/// Future returned by [close] method.
///
/// [close]: mezzenger::Close::close
pub struct Close<'a, Message> {
    transport: &'a Transport<Message>,
    terminated: bool,
}

impl<'a, Message> Future for Close<'a, Message> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Pending
        } else {
            self.transport.close();
            self.terminated = true;
            Poll::Ready(())
        }
    }
}

impl<'a, Message> FusedFuture for Close<'a, Message> {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<Message> mezzenger::Send<Message> for Transport<Message>
where
    Message: Clone,
{
    type Error = Infallible;

    type Output<'a> = Send<'a, Message> where Self: 'a;

    fn send<'a>(&'a self, message: &Message) -> Self::Output<'a> {
        Send {
            transport: self,
            message: Some(message.clone()),
        }
    }
}

/// Future returned by [send] method.
///
/// [send]: mezzenger::Send::send
pub struct Send<'a, Message> {
    transport: &'a Transport<Message>,
    message: Option<Message>,
}

impl<'a, Message> Unpin for Send<'a, Message> {}

impl<'a, Message> Future for Send<'a, Message> {
    type Output = Result<(), mezzenger::Error<Infallible>>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            use mezzenger::Close;
            if self.transport.is_closed() {
                self.message.take();
                Poll::Ready(Err(mezzenger::Error::Closed))
            } else {
                self.transport
                    .state
                    .lock()
                    .buffer
                    .push_front(self.message.take().unwrap());
                self.transport.wake_next();
                Poll::Ready(Ok(()))
            }
        } else {
            Poll::Pending
        }
    }
}

impl<'a, Message> FusedFuture for Send<'a, Message> {
    fn is_terminated(&self) -> bool {
        self.message.is_none()
    }
}

impl<Message> mezzenger::Receive<Message> for Transport<Message> {
    type Error = Infallible;

    type Output<'a> = Receive<'a, Message> where Message: 'a;

    fn receive(&self) -> Self::Output<'_> {
        Receive {
            transport: self,
            terminated: false,
            waker: None,
        }
    }
}

/// Future returned by [receive] method.
///
/// [receive]: mezzenger::Receive::receive
pub struct Receive<'a, Message> {
    transport: &'a Transport<Message>,
    terminated: bool,
    waker: Option<Arc<Mutex<ReceiveWaker>>>,
}

impl<'a, Message> Drop for Receive<'a, Message> {
    fn drop(&mut self) {
        // We were woken but didn't receive anything, wake up another
        if self.waker.take().map_or(false, |waker| waker.lock().woken) {
            self.transport.wake_next();
        }
    }
}

impl<'a, Message> Unpin for Receive<'a, Message> {}

struct ReceiveWaker {
    waker: Waker,
    woken: bool,
}

impl ReceiveWaker {
    fn new(waker: Waker) -> Self {
        ReceiveWaker {
            waker,
            woken: false,
        }
    }

    fn update(&mut self, waker: &Waker) {
        if !self.waker.will_wake(waker) {
            self.waker = waker.clone();
        }
    }
}

impl<'a, Message> Future for Receive<'a, Message> {
    type Output = Result<Message, mezzenger::Error<Infallible>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Pending
        } else {
            let mut state = self.transport.state.lock();
            match state.buffer.pop_back() {
                Some(message) => {
                    self.terminated = true;
                    self.waker = None;
                    Poll::Ready(Ok(message))
                }
                None => {
                    if state.closed {
                        self.terminated = true;
                        Poll::Ready(Err(mezzenger::Error::Closed))
                    } else {
                        if let Some(waker) = &self.waker {
                            let mut waker = waker.lock();
                            waker.update(cx.waker());
                            waker.woken = false;
                        } else {
                            let waker = Arc::new(Mutex::new(ReceiveWaker::new(cx.waker().clone())));
                            self.waker = Some(waker);
                        }
                        state
                            .wakers
                            .push_front(Arc::downgrade(self.waker.as_ref().unwrap()));
                        Poll::Pending
                    }
                }
            }
        }
    }
}

impl<'a, Message> FusedFuture for Receive<'a, Message> {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}
