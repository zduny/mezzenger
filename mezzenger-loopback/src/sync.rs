// todo - use tokio channels

use std::convert::Infallible;

use mezzenger_common::{
    sync::{Close, Receive, State},
    Send,
};
use parking_lot::Mutex;

/// Transport storing messages in local queue.
///
/// It does not send messages anywhere (to other side of transport) - messages 'received'
/// from this transport are the same messages that were 'sent' into it locally.
///
/// **NOTE**: do NOT use this transport to send data between threads or async tasks,
/// use appropriate channels instead as they will most likely be much more performant.
pub struct Transport<Message> {
    state: Mutex<State<Message, Infallible>>,
}

impl<Message> Transport<Message> {
    /// Creates new dummy transport.
    pub fn new() -> Self {
        Transport {
            state: Mutex::new(State::new()),
        }
    }
}

impl<Message> Default for Transport<Message> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Sender {}

impl<Message> mezzenger_common::Sender<Transport<Message>, Message, Infallible> for Sender
where
    Message: Clone,
{
    fn send(
        transport: &Transport<Message>,
        message: &Message,
    ) -> Result<(), mezzenger::Error<Infallible>> {
        use mezzenger::Close;
        if transport.is_closed() {
            Err(mezzenger::Error::Closed)
        } else {
            transport.state.lock().message(message.clone());
            Ok(())
        }
    }
}

impl<Message> mezzenger::Send<Message> for Transport<Message>
where
    Message: Clone,
{
    type Error = Infallible;

    type Output<'a> = Send<'a, Transport<Message>, Message, Infallible, Sender> where Self: 'a;

    fn send<'s, 'm>(&'s self, message: &'m Message) -> Self::Output<'s>
    where
        'm: 's {
        Send::new(self, message)
    }
}

impl<Message> mezzenger::Receive<Message> for Transport<Message> {
    type Error = Infallible;

    type Output<'a> = Receive<'a, Message, Infallible>
    where
        Self: 'a;

    fn receive(&self) -> Self::Output<'_> {
        Receive::new(&self.state)
    }
}

impl<Message> mezzenger::Close for Transport<Message> {
    type Output<'a> = Close<'a, Message, Infallible> where Self: 'a;

    fn close(&self) -> Self::Output<'_> {
        Close::new(&self.state)
    }

    fn is_closed(&self) -> bool {
        self.state.lock().closed
    }
}

impl<Message> mezzenger::Reliable for Transport<Message> {}

impl<Message> mezzenger::Order for Transport<Message> {}
