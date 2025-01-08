use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use rand::Rng;

use crate::app::Color;
use crate::app::ColorStateMachine;
use crate::app::piclient::PiClient;
use crate::app::piclient::SenseHat;
use crate::log_impl::in_memory::InMemoryLog;
use crate::network::in_memory::InMemoryNetwork;
use crate::raft::membership::Membership;
use crate::raft::message::Message;
use crate::startup::server::setup_node;
use crate::raft::message;
use crate::raft::message::ClientResponder;
use crate::raft::message::ClientResponse;
use crate::raft::membership::Address;
use std::collections::HashMap;
use crate::raft::membership::NodeId;

pub struct PrintlnResponder;
impl ClientResponder for PrintlnResponder {
    fn respond(&mut self, msg: ClientResponse) {
        println!("======= {:?} =======", msg);
    }
}

/// drive a bunch of activity for a while using the simulated network, including isolating nodes
/// This doesn't actually assert anything, but watching the cluster perform, reading through the
/// logs, etc. is useful.
pub fn run_for_a_while() {

    // ==========================================
    // Setup Cluster
    // ==========================================
    let network = InMemoryNetwork::<Color>::new();

    let pi_addr: SocketAddr = "10.0.1.2:12345".parse().expect("cannot parse pi addr");
    let sense_hat = SenseHat(pi_addr);

    let endpoint1 = Address("127.0.0.1:10001".to_string());
    let endpoint2 = Address("127.0.0.1:10002".to_string());
    let endpoint3 = Address("127.0.0.1:10003".to_string());
    let mut members = HashMap::new();
    members.insert(NodeId(1), endpoint1.clone());
    members.insert(NodeId(2), endpoint2.clone());
    members.insert(NodeId(3), endpoint3.clone());
    let membership = Membership::Stable(members.clone());

    setup_node(NodeId(1), membership.clone(), &network, ColorStateMachine::new(PiClient::new(1, (0, 2), sense_hat)), InMemoryLog::new());
    setup_node(NodeId(2), membership.clone(), &network, ColorStateMachine::new(PiClient::new(2, (1, 2), sense_hat)), InMemoryLog::new());
    setup_node(NodeId(3), membership.clone(), &network, ColorStateMachine::new(PiClient::new(3, (2, 2), sense_hat)), InMemoryLog::new());

    // ==========================================
    // Drive the test cluster.  Send commands and
    // isolate/unisolate nodes
    // ==========================================
    let start = Instant::now();
    let mut last_color_update = Instant::now();
    let mut last_block_action = Instant::now();
    let mut currently_blocked = None;

    let mut rng = rand::thread_rng();
    while start.elapsed() < Duration::from_secs(300) {

        let member_list = membership.as_set().iter().map(|id| membership.address_of(*id).unwrap().clone() ).collect::<Vec<Address>>();
        network.process_some();

        // sometimes block or unblock nodes
        if last_block_action.elapsed() > Duration::from_secs(30) {
            last_block_action = Instant::now();
            if let Some(victim) = currently_blocked {
                println!("unblocking {:?}", victim);
                network.unblock(victim);
                currently_blocked = None;
            } else {
                let victim_idx = rng.gen_range(0, member_list.len());
                let victim = member_list.get(victim_idx ).unwrap().clone();
                network.block(victim.clone());
                currently_blocked = Some(victim.clone());
                println!("blocking {:?}", victim);
            }
        }

        if last_color_update.elapsed() > Duration::from_secs(2) {
            last_color_update = Instant::now();
            for member in member_list {
                let r = rng.gen_range(0, 255);
                let g = rng.gen_range(0, 255);
                let b = rng.gen_range(0, 255);
                let color = Color::new(r, g, b);
                network.send(Message(member.clone(), message::Contents::Command( color, Box::new(PrintlnResponder))));
            }
        }
    }
}
