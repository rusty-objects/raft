use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use crate::raft::message;
use crate::raft::message::Message;
use crate::raft::membership::Address;

pub mod in_memory;
pub mod real;

pub trait Network {
    type Cmd;

    fn register(&self, member: Address) -> (Receiver<message::Contents<Self::Cmd>>, Sender<Message<Self::Cmd>>);
}
