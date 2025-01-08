use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use crate::app::Color;
use crate::network::Network;
use crate::raft::membership::Address;
use crate::raft::message;
use crate::raft::message::AppendAck;
use crate::raft::message::AppendEntries;
use crate::raft::message::AppendNack;
use crate::raft::message::ClientResponder;
use crate::raft::message::ClientResponse;
use crate::raft::message::Contents;
use crate::raft::message::Message;
use crate::raft::message::RequestVote;
use crate::raft::message::VoteResponse;

const APPEND_ENTRIES: &'static str = "APP ";
const     APPEND_ACK: &'static str = "ACK ";
const    APPEND_NACK: &'static str = "NCK ";
const   REQUEST_VOTE: &'static str = "REQ ";
const  VOTE_RESPONSE: &'static str = "VOT ";
const        COMMAND: &'static str = "CMD ";

pub struct RealNetwork<Cmd> {
    endpoint: Address,
    send_to_member: Arc<Mutex<Sender<message::Contents<Cmd>>>>,
    recv_from_member: Arc<Mutex<Receiver<Message<Cmd>>>>,
    member_side_recv: Mutex<Option<Receiver<message::Contents<Cmd>>>>,
    member_side_send: Mutex<Option<Sender<Message<Cmd>>>>,
}
impl RealNetwork<Color> {
    pub fn new(endpoint: Address) -> Self {
        let (send_to_member, member_side_recv) = mpsc::channel();
        let (member_side_send, recv_from_member) = mpsc::channel();
        Self {
            endpoint,
            send_to_member: Arc::new(Mutex::new(send_to_member)),
            recv_from_member: Arc::new(Mutex::new(recv_from_member)),
            member_side_recv: Mutex::new(Some(member_side_recv)),
            member_side_send: Mutex::new(Some(member_side_send)),
        }
    }

    /// Create a listener to handle incoming messages and a sender to send outgoing messages
    pub fn run(self) {

        // create sender thread
        let rcvr = Arc::clone( &self.recv_from_member );
        let endpoint_clone = self.endpoint.clone();
        let sender_thread = thread::spawn(move || {
            let start = Instant::now();
            loop {
                match rcvr.lock().unwrap().recv() {
                    Ok(msg) => {
                        let dest = msg.0.clone();

                        println!("out [{:?} {:?}->{:?}] {:?}]", start.elapsed(), endpoint_clone, dest, msg.1);

                        match TcpStream::connect(dest.0) {
                            Ok(mut stream) => {
                                match msg.1 {
                                    Contents::RequestVote(c) => {
                                        stream.write(REQUEST_VOTE.as_bytes()).unwrap();
                                        serde_json::to_writer(stream, &c).unwrap();
                                    },
                                    Contents::VoteResponse(c) => {
                                        stream.write(VOTE_RESPONSE.as_bytes()).unwrap();
                                        serde_json::to_writer(stream, &c).unwrap();
                                    },
                                    Contents::AppendEntries(c) => {
                                        stream.write(APPEND_ENTRIES.as_bytes()).unwrap();
                                        serde_json::to_writer(stream, &c).unwrap();
                                    },
                                    Contents::AppendAck(c) => {
                                        stream.write(APPEND_ACK.as_bytes()).unwrap();
                                        serde_json::to_writer(stream, &c).unwrap();
                                    },
                                    Contents::AppendNack(c) => {
                                        stream.write(APPEND_NACK.as_bytes()).unwrap();
                                        serde_json::to_writer(stream, &c).unwrap();
                                    },
                                    Contents::Command(_, _) => continue,
                                }
                            },
                            Err(e) => {
                                println!("Failed to connect: {}", e);
                            }
                        }
                    },
                    Err(_) => break,
                }
            }
        });

        // create listener thread
        let to_member = Arc::clone( &self.send_to_member );
        let endpoint = self.endpoint.clone();
        let start = Instant::now();
        let listener_thread = thread::spawn(move || {
            let listener = TcpListener::bind(endpoint.clone().0).unwrap();
            println!("Server listening on {:?}", endpoint);
            let mut buf = [0u8; 4];
            for accept in listener.incoming() {
                match accept {
                    // https://stackoverflow.com/questions/47146114/deserializing-newline-delimited-json-from-a-socket-using-serde
                    // https://docs.serde.rs/serde_json/ser/fn.to_writer.html
                    // https://docs.serde.rs/serde_json/de/fn.from_reader.html
                    Ok(mut stream) => {
                        stream.read_exact(&mut buf).unwrap();

                        let contents = match String::from_utf8_lossy(&buf[..]).as_ref() {
                            APPEND_ENTRIES => {
                                let inner: AppendEntries<Color> = serde_json::from_reader(stream).unwrap();
                                message::Contents::AppendEntries(inner)
                            },
                            APPEND_ACK => {
                                let inner: AppendAck = serde_json::from_reader(stream).unwrap();
                                message::Contents::AppendAck(inner)
                            },
                            APPEND_NACK => {
                                let inner: AppendNack = serde_json::from_reader(stream).unwrap();
                                message::Contents::AppendNack(inner)
                            },
                            REQUEST_VOTE => {
                                let inner: RequestVote = serde_json::from_reader(stream).unwrap();
                                message::Contents::RequestVote(inner)
                            },
                            VOTE_RESPONSE => {
                                let inner: VoteResponse = serde_json::from_reader(stream).unwrap();
                                message::Contents::VoteResponse(inner)
                            },
                            COMMAND => {
                                let inner: Color = serde_json::from_reader(&mut stream).unwrap();
                                message::Contents::Command(inner, Box::new(StreamResponder(stream)))
                            },

                            _ => continue,
                        };
                        println!("in  [{:?} {:?}] {:?}", start.elapsed(), endpoint, contents);
                        to_member.lock().unwrap().send(contents).unwrap();
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
        });

        listener_thread.join().unwrap();
        sender_thread.join().unwrap();
    }
}
impl<Cmd> Network for RealNetwork<Cmd> {
    type Cmd = Cmd;

    fn register(&self, _member: Address) -> (Receiver<message::Contents<Self::Cmd>>, Sender<Message<Self::Cmd>>) {
        let member_side_recv = self.member_side_recv.lock().unwrap().take();
        let member_side_send = self.member_side_send.lock().unwrap().take();
        (member_side_recv.unwrap(), member_side_send.unwrap())
    }
}

pub struct StreamResponder(TcpStream);
impl ClientResponder for StreamResponder {
    fn respond(&mut self, msg: ClientResponse) {
        serde_json::to_writer(&mut self.0, &msg).unwrap();
    }
}
