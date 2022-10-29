use futures::{future::FusedFuture, Future};
use std::{
    cell::RefCell,
    collections::VecDeque,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker}, marker::PhantomData,
};

/// Transport state.
///
/// **NOTE**: It only supports sending/closing in immediate synchronous fashion - if your transport
/// needs to implement sending/closing that is asynchronous in nature then you either have to ignore
/// sending/closing infrastructure implemented here and implement sending/closing yourself or not use
/// `mezzenger-common` at all and implement your entire transport manually.
pub struct State<Message, Error> {
    pub queue: VecDeque<Result<Message, Error>>,
    pub wakers: VecDeque<Weak<RefCell<ReceiveWaker>>>,
    pub closed: bool,
}

impl<Message, Error> State<Message, Error> {
    /// Create new state for transport.
    pub fn new() -> Self {
        State {
            queue: VecDeque::new(),
            wakers: VecDeque::new(),
            closed: false,
        }
    }

    /// Push received message into transport queue.
    pub fn message(&mut self, message: Message) {
        self.queue.push_front(Ok(message))
    }

    /// Push error into transport queue.
    pub fn error(&mut self, error: Error) {
        self.queue.push_front(Err(error))
    }

    /// Wake next receive waker.
    pub fn wake_next(&mut self) {
        while let Some(waker) = self.wakers.pop_front() {
            if let Some(waker) = waker.upgrade() {
                let mut waker = waker.borrow_mut();
                waker.woken = true;
                waker.waker.wake_by_ref();
                break;
            }
        }
    }

    /// Signal that transport was closed.
    pub fn close(&mut self) {
        self.closed = true;
        self.wake_next();
    }
}

impl<Message, Error> Default for State<Message, Error> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Error> Drop for State<Message, Error> {
    fn drop(&mut self) {
        self.close();
    }
}

/// Future returned by [receive] method.
///
/// [receive]: mezzenger::Receive::receive
pub struct Receive<'a, Message, Error> {
    state: &'a RefCell<State<Message, Error>>,
    terminated: bool,
    waker: Option<Rc<RefCell<ReceiveWaker>>>,
}

impl<'a, Message, Error> Receive<'a, Message, Error> {
    /// Create new future for [receive] method.
    ///
    /// [receive]: mezzenger::Receive::receive
    pub fn new(state: &'a RefCell<State<Message, Error>>) -> Self {
        Receive {
            state,
            terminated: false,
            waker: None,
        }
    }
}

impl<'a, Message, Error> Drop for Receive<'a, Message, Error> {
    fn drop(&mut self) {
        // We were woken but didn't receive anything, wake up another
        if self
            .waker
            .take()
            .map_or(false, |waker| waker.borrow().woken)
        {
            self.state.borrow_mut().wake_next();
        }
    }
}

impl<'a, Message, Error> Unpin for Receive<'a, Message, Error> {}

pub struct ReceiveWaker {
    pub waker: Waker,
    pub woken: bool,
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

impl<'a, Message, Error> Future for Receive<'a, Message, Error> {
    type Output = Result<Message, mezzenger::Error<Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Pending
        } else {
            let mut state = self.state.borrow_mut();
            match state.queue.pop_back() {
                Some(item) => {
                    self.terminated = true;
                    self.waker = None;
                    Poll::Ready(item.map_err(|error| mezzenger::Error::Other(error)))
                }
                None => {
                    if state.closed {
                        self.terminated = true;
                        Poll::Ready(Err(mezzenger::Error::Closed))
                    } else {
                        if let Some(waker) = &self.waker {
                            let mut waker = waker.borrow_mut();
                            waker.update(cx.waker());
                            waker.woken = false;
                        } else {
                            let waker =
                                Rc::new(RefCell::new(ReceiveWaker::new(cx.waker().clone())));
                            self.waker = Some(waker);
                        }
                        state
                            .wakers
                            .push_front(Rc::downgrade(self.waker.as_ref().unwrap()));
                        Poll::Pending
                    }
                }
            }
        }
    }
}

impl<'a, Message, Error> FusedFuture for Receive<'a, Message, Error> {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

pub trait Closer<Transport> {
    fn close(transport: &Transport);
}

/// Future returned by [close] method.
///
/// [close]: mezzenger::Close::close
pub struct Close<'a, Transport, Message, Error, C>
where
    C: Closer<Transport>,
{
    transport: &'a Transport,
    state: &'a RefCell<State<Message, Error>>,
    terminated: bool,
    _closer: PhantomData<C>,
}

impl<'a, Transport, Message, Error, C> Close<'a, Transport, Message, Error, C>
where
    C: Closer<Transport>,
{
    /// Create new future for [close] method.
    ///
    /// [close]: mezzenger::Close::close
    pub fn new(
        transport: &'a Transport,
        state: &'a RefCell<State<Message, Error>>,
    ) -> Self {
        Close {
            transport,
            state,
            terminated: false,
            _closer: PhantomData,
        }
    }
}

impl<'a, Transport, Message, Error, C> Future for Close<'a, Transport, Message, Error, C>
where
    C: Closer<Transport>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Ready(())
        } else {
            C::close(&self.transport);
            self.state.borrow_mut().close();
            self.terminated = true;
            Poll::Ready(())
        }
    }
}

impl<'a, Transport, Message, Error, C> FusedFuture for Close<'a, Transport, Message, Error, C>
where
    C: Closer<Transport>,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<'a, Transport, Message, Error, C> Unpin for Close<'a, Transport, Message, Error, C> where
    C: Closer<Transport>
{
}
