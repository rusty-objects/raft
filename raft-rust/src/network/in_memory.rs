use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::TryRecvError;
use std::sync::Mutex;
use std::time::Instant;

use crate::network::Network;
use crate::raft::message;
use crate::raft::message::Message;
use crate::raft::membership::Address;

// Interesting implementation note.  Sender and Receiver are Send but not Sync.
// So if InMemoryNetwork is going to be shared across threads (e.g. for register),
// then we'll get a compile failure because you cannot share InMemoryNetwork across
// threads.  Either member_nic_out and to_route needs to be wrapped in some sort of
// lock (Mutex or RwLock), or InMemoryNetwork cannot be shared across threads (e.g.
// can't be shared by wrapping in an Arc then cloned).
pub struct InMemoryNetwork<Cmd> {
    from_members: Mutex<HashMap<Address, Receiver<Message<Cmd>>>>,
    to_members: Mutex<HashMap<Address, Sender<message::Contents<Cmd>>>>,
    blocked: Mutex<HashSet<Address>>,
    start: Instant,
}

impl<Cmd> InMemoryNetwork<Cmd>
where Cmd: Debug {
    pub fn new() -> Self {
        Self {
            to_members: Mutex::new(HashMap::new()),
            from_members: Mutex::new(HashMap::new()),
            blocked: Mutex::new(HashSet::new()),
            start: Instant::now(),
        }
    }

    pub fn process_some(&self) -> bool {
        let mut did_work = false;
        let from_map = self.from_members.lock().unwrap();
        let to_map = self.to_members.lock().unwrap();
        for (member, recv) in from_map.iter() {
            let result = recv.try_recv();
            match result {
                Ok(Message(dest, contents)) => {
                    did_work = true;
                    let maybe_sender = to_map.get(&dest);
                    let blocked_list = self.blocked.lock().unwrap();
                    let blocked = blocked_list.contains(&dest) || blocked_list.contains(&member);
                    let dropped = match blocked {
                        true => " <DROPPED>",
                        false => "",
                    };

                    // if the dest is unknown, print it anyway
                    if maybe_sender.is_none() {
                        println!("[{:?} {:?}->{:?}?] {:?}", self.start.elapsed(), member, dest.0, contents);
                        continue;
                    }
                    let sender = maybe_sender.unwrap();
                    println!("[{:?} {:?}->{:?}{}] {:?}", self.start.elapsed(), member, dest.0, dropped, contents);

                    if !blocked {
                        sender.send(contents).unwrap();
                    }
                },
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => return false,
            }
        }
        did_work
    }


    pub fn block(&self, endpoint: Address) {
        self.blocked.lock().unwrap().insert(endpoint);
    }

    pub fn unblock(&self, endpoint: Address) {
        self.blocked.lock().unwrap().remove(&endpoint);
    }

    pub fn send(&self, msg: Message<Cmd>) {
        let members = self.to_members.lock().unwrap();
        let channel = members.get(&msg.0).unwrap();

        channel.send(msg.1).unwrap();
    }
}

impl<Cmd> Network for InMemoryNetwork<Cmd> {
    type Cmd = Cmd;

    fn register(&self, member: Address) -> (Receiver<message::Contents<Self::Cmd>>, Sender<Message<Self::Cmd>>) {
        let (send_to_member, member_side_recv) = mpsc::channel();
        let (member_side_send, recv_from_member) = mpsc::channel();
        self.from_members.lock().unwrap().insert(member.clone(), recv_from_member);
        self.to_members.lock().unwrap().insert(member.clone(), send_to_member);
        (member_side_recv, member_side_send)
    }

}