use std::thread;
use std::thread::JoinHandle;

use crate::network::Network;
use crate::raft::app::ApplicationStateMachine;
use crate::raft::event_loop;
use crate::raft::log::Log;
use crate::raft::membership::Membership;
use crate::raft::membership::NodeId;
use crate::raft::state::RaftState;

/// Starts up a server, real or otherwise
pub fn setup_node<Cmd, A, L>(node_id: NodeId, membership: Membership, network: &impl Network<Cmd=Cmd>, application: A, log: L) -> JoinHandle<()>
where Cmd: 'static + Send,
      A: 'static + ApplicationStateMachine<Cmd> + Send,
      L: 'static + Log<Cmd> + Send {

    // the assumption here is that we're already in membership.  panic during setup if we're not.
    let address = membership.address_of(node_id).unwrap().clone();
    let (receiver, sender) = network.register(address );
    thread::spawn(move || {
        let state = RaftState::new(node_id, log, application, membership);
        event_loop(receiver, sender, state);
    })
}
