//! This module is the core raft algorithm, and is agnostic of the specific problem domain.
//! This file contains most of the core raft algorithm, though some pieces are shared between
//! sub-modules.  For example the logic for determining if a majority vote has been reached on
//! an in-flight membership transition is invoked from here but implemented in the membership
//! sub-module.

use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;

use crate::raft::app::ApplicationStateMachine;
use crate::raft::log::Log;
use crate::raft::log::LogEntry;
use crate::raft::log::LogPosition;
use crate::raft::log::Payload;
use crate::raft::membership::NodeId;
use crate::raft::message::AppendAck;
use crate::raft::message::AppendEntries;
use crate::raft::message::AppendNack;
use crate::raft::message::ClientResponder;
use crate::raft::message::ClientResponse;
use crate::raft::message::Message;
use crate::raft::message::RequestVote;
use crate::raft::message::Vote;
use crate::raft::message::VoteResponse;
use crate::raft::state::RaftState;
use crate::raft::state::Role;

pub mod app;
pub mod message;
pub mod log;
pub mod membership;
pub mod state;

/// main event loop.  runs forever until the supplied inbound Receiver reaches EOF.
pub fn event_loop<C, L, A>(receiver: Receiver<message::Contents<C>>, sender: Sender<Message<C>>, mut state: RaftState<L, A>)
where L: Log<C>, A: ApplicationStateMachine<C> {
    loop {
        match receiver.recv_timeout(state.time_until_next_interesting_event()) {
            Ok(message::Contents::RequestVote(msg)) => handle_request_vote(msg, &mut state, &sender),
            Ok(message::Contents::VoteResponse(msg)) => handle_vote_response(msg, &mut state, &sender),
            Ok(message::Contents::AppendEntries(msg)) => handle_append_entries(msg, &mut state, &sender),
            Ok(message::Contents::AppendAck(msg)) => handle_append_ack(msg, &mut state),
            Ok(message::Contents::AppendNack(msg)) => handle_append_nack(msg, &mut state, &sender),
            Ok(message::Contents::Command(msg, responder)) => handle_command(msg, responder, &mut state, &sender),
            Err(RecvTimeoutError::Timeout) => handle_timer(&mut state, &sender),
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Handles events from timers. We don't want to assume that the presence of this event is
/// significant, so anything that depends on timing conditions should specifically check
/// the timing before deciding to act as if some duration has passed.  E.g. don't just
/// assume that because the handler was called that an election timeout occurred.
pub fn handle_timer<Cmd, Log, Application>(state: &mut RaftState<Log, Application>, sender: &Sender<Message<Cmd>>)
where Log: log::Log<Cmd>, Application: ApplicationStateMachine<Cmd> {

    match state.my_role() {
        Role::Leader(leader_state) => {
            if leader_state.is_past_heartbeat_timeout() {
                broadcast_log(state, sender);  // heartbeat.  will send empty entries to caught up followers.
            }

            // TODO consider having leaders turn_into_follower if it's been too long since a commit
        },
        Role::Follower(_) => {
            if state.is_past_election_timeout() {
                start_election(state, sender);
            }
        },
        Role::Candidate(_) => {
            if state.is_past_election_timeout() {
                start_election(state, sender);
            }
        },
    }
}

pub fn start_election<C, L, A>(state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: log::Log<C> {
    state.turn_into_candidate();

    let last_position = state.log().last();
    let request_vote = RequestVote {
        term: state.term(),
        candidate: state.myself(),
        last_position,
    };

    state.membership().as_set().iter().for_each(|member| {
        if *member != state.myself() {
            if let Some(addr) = state.membership().address_of(*member) {
                sender.send(Message(addr.clone(), message::Contents::RequestVote(request_vote))).unwrap();
            }
        }
    });

    state.reset_election_timeout();
}

/// Logic when we receive a RequestVote RPC.  This is depicted in Figure 2 of the raft paper,
/// and described in detail in the sections documented below, as well as Sections 5.2 and 5.4.1.
pub fn handle_request_vote<Cmd, Log, A>(
    request_vote: RequestVote, state: &mut RaftState<Log, A>, sender: &Sender<Message<Cmd>>)
where Log: log::Log<Cmd> {

    // See section 5.1: if a message has a larger term, adopt that term and become a follower
    if state.term() < request_vote.term {
        state.turn_into_follower(request_vote.term);
    }

    // unless we're a candidate, there appears to be an election in process.  let's not ruin it by using
    // our previous election timeout and stomping on top of the in-flight election.
    if !state.is_candidate() {
        state.reset_election_timeout();
    }

    // If the candidate's term is less than ours, they're too stale to be leader.
    let vote = if request_vote.term < state.term() {
        Vote::Deny
    } else {
        // above, we had reset our term if it was ahead
        assert_eq!(request_vote.term, state.term(), "invariant violation: term is behind event term");

        let already_voted_for_someone_else = match state.voted_for() {
            None => false,
            Some(candidate) => candidate != request_vote.candidate,
        };

        match already_voted_for_someone_else {
            true => Vote::Deny,
            false => {
                // grant the vote if the candidate's logs are up to date.

                let my_last_position = state.log().last();
                let candidate_last_position = request_vote.last_position;

                // the definition of a log being up to date is defined in section 5.4.1
                if candidate_last_position.is_up_to_date(my_last_position) {
                    state.vote_for(request_vote.candidate);
                    Vote::Grant
                } else {
                    Vote::Deny
                }
            },
        }
    };

    // we always include _our_ term in outbound messages.  The term will match what the candidate
    // sent unless the candidate was stale, in which case the term will be ahead of there's and
    // the vote_result will be a denial.
    let vote_response = VoteResponse{
        term: state.term(),
        voter: state.myself(),
        candidate: request_vote.candidate,
        vote
    };

    if let Some(addr) = state.membership().address_of(request_vote.candidate ) {
        let outbound_message = Message(addr.clone(), message::Contents::VoteResponse(vote_response));
        sender.send(outbound_message).unwrap();
    }
}

pub fn handle_vote_response<C, L, A>(
    vote_response: VoteResponse, state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C>, A: ApplicationStateMachine<C> {

    assert_eq!(vote_response.candidate, state.myself(), "received vote response for someone else?");

    // See section 5.1: if a message has a larger term, adopt that term and become a follower
    // not sure how we'd get a vote_response for a larger term than we sent the vote for
    if state.term() < vote_response.term {
        state.turn_into_follower(vote_response.term);
    }

    // in case there's any weird edge cases around a node going offline and coming
    // back as someone else in the middle of a vote
    if vote_response.candidate != state.myself() {
        return;
    }

    if state.is_candidate() && state.term() == vote_response.term && vote_response.vote == Vote::Grant {
        state.candidate_state_mut().insert_yes_vote(vote_response.voter);
        if state.candidate_has_won_election() {
            state.turn_into_leader();

            // create a noop entry, first thing after gaining leadership
            let last = state.log().last();
            let entry = LogEntry {
                position: LogPosition(last.0.next(), state.term()),
                payload: Payload::Noop(state.myself()),
            };
            state.log_mut().append_if(vec![entry], last).expect("leader should be able to append noop");

            broadcast_log(state, sender);

        }
    }
}

/// Sends the appropriate list of log entries to the specified member.  This is called by leaders, and
/// figures out exactly what needs to be sent to the recipient of interest.
fn transmit_log<C, L, A>(to: NodeId, state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C> {
    let maybe_addr = state.membership().address_of(to);
    if to != state.myself() && maybe_addr.is_some() {
        let addr = maybe_addr.unwrap().clone();

        let mut entries = Vec::new();
        let start_index = state.leader_state().next_index_for(to);
        state.log().scan(start_index, None, &mut |entry: LogEntry<C>| entries.push(entry));
        let prev_pos = LogPosition(start_index.prev(), state.log().term_for(start_index.prev()));
        let commit_index = state.commit_index();

        let msg = AppendEntries::new(state.term(), state.myself(), prev_pos, commit_index, entries);
        let outbound = Message(addr, message::Contents::AppendEntries(msg));
        sender.send(outbound).unwrap();
    }
}

/// as a leader, we'll send AppendEntries to everyone when we heartbeat or a meaningful change occurs
pub fn broadcast_log<C, L, A>( state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C> {
    // append to all
    state.membership().as_set().iter().for_each(|member| transmit_log(*member, state, sender));

    // reset the heartbeat timer.  this should happen every time we broadcast to all
    state.leader_state_mut().reset_heartbeat_timeout();
}

pub fn handle_append_entries<C, L, A>(
    append_entries: AppendEntries<C>, state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C>, A: ApplicationStateMachine<C> {

    // See section 5.1: if a message has a larger term, adopt that term and become a follower
    // not sure how we'd get a vote_response for a larger term than we sent the vote for
    if state.term() < append_entries.term {
        state.turn_into_follower(append_entries.term);
    }

    // See section 5.2: special in the case of append_entries, if the term is equal to ours
    // and we're a candidate, it would imply that we've lost the election this round.
    if state.is_candidate() && state.term() == append_entries.term {
        state.turn_into_follower(append_entries.term)
    }

    // See section 5.2: any append_entries from the past are rejected.  here we'll just drop.  the sender
    // will soon realize it's no longer the leader once it gets a vote request or append entries from the
    // next leader/candidate.  We don't have to tell it here.
    if state.term() > append_entries.term {
        return;
    }

    // if we're here, we have the same term as the message, and will attempt to append the data as long
    // as our log has the right previous entry.
    assert_eq!(state.term(), append_entries.term);

    // also if we're here, the sender is the leader, so we'll mark it
    state.follower_state_mut().update_leader(append_entries.leader);

    let result = state.log_mut().append_if(append_entries.entries, append_entries.previous_position);
    let contents = match result {
        Ok(new_latest_position) => {
            // only apply commands if we've successfully appended.  Otherwise our log might have incorrect uncommitted
            // entries in it whose indices match the leader but whose contents don't.  As far as I could tell this isn't
            // specifically discussed in the paper, but it's implied in Figure 2's ordering of receiver implementation.
            state.maybe_set_commit_index(append_entries.commit_index);
            state.apply_commands();
            message::Contents::AppendAck(AppendAck(state.myself(), new_latest_position))
        },
        Err(_) => message::Contents::AppendNack(AppendNack(state.term(), append_entries.previous_position.0, state.myself())),
    };

    /*
     * TODO
     * Will this work in the case of a membership change when the leader is in the old group?
     * In that case their last role as the leader is to commit the new membership with only the
     * new group, then they give up leadership.  But here, that will only be able to succeed on
     * the new group.  The old group won't have the leader's address to ACK back, and the leader
     * can't consider the closing of the old membership committed until it hears back from everyone.
     * Perhaps the responses back should be based off the return address in the message rather
     * than the membership.
     */
    if let Some(addr) = state.membership().address_of(append_entries.leader) {
        sender.send(Message(addr.clone(), contents)).unwrap();
    }

    // since we're here, whether or not we actually successfully appended anything, we at least have heard
    // from the leader.   so let's do some housekeeping:
    state.reset_election_timeout();
}

pub fn handle_append_ack<C, L, A>(command_ack: AppendAck, state: &mut RaftState<L, A>)
    where L: Log<C>, A: ApplicationStateMachine<C> {

    // if the response is not from our current term, it's possible it was a delayed message from a previous
    // leadership and we'll just ignore it.   We shouldn't be getting ACKs from the future, as they're only
    // in response to our AppendEntries messages, and we don't have a later term than our current one.
    if (command_ack.1).1 != state.term() {
        return;
    }

    let index = (command_ack.1).0;

    // upon success, we'll do some housekeeping
    state.leader_state_mut().set_next_index_for(command_ack.0, index.next());
    state.leader_state_mut().set_match_index_for(command_ack.0, index);

    // maybe we have enough to update our commit index
    // TODO this sort of call pattern is weird.  why are we passing in log?
    let maybe_new_commit_index = state.leader_state().commit_index(state.log());
    match maybe_new_commit_index {
        None => (),
        Some(commit_index) => state.maybe_set_commit_index(commit_index),
    };

    // at this point, as the leader, we can apply commands up to the commit index.  We do it here because
    // the only opportunity to advance our commit point comes from member ack messages.
    state.apply_commands();
}

pub fn handle_append_nack<C, L, A>(
    command_nack: AppendNack, state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C> {

    // if the response is not from our current term, it's possible it was a delayed message from a previous
    // leadership and we'll just ignore it.   We shouldn't be getting NACKs from the future, as they're only
    // in response to our AppendEntries messages, and we don't have a later term than our current one.
    if command_nack.0 != state.term() {
        return;
    }

    // the index we sent wasn't any good.  let's try a previous one.
    // TODO only do this if the member's match_index hasn't superceded this index.  Otherwise a delayed NACK from the
    // TODO past could end up resetting the members state.  It wouldn't be incorrect to send the state back in time like
    // TODO this, but it's sub-optimal.
    // We set the next index to the index in the nack, which is the previous to the one that we sent
    state.leader_state_mut().set_next_index_for(command_nack.2, command_nack.1);

    // now since we know the recipient rejected our last message, let's retry immediately with the newer index
    // if they reject that, we'll decrement the index and retry again.  Eventually we'll succeed (since everyone
    // agrees on the "null" entry at index 0).
    transmit_log(command_nack.2, state, sender);
}

pub fn handle_command<C, L, A>(
    command: C, mut responder: Box<dyn 'static + ClientResponder + Send>, state: &mut RaftState<L, A>, sender: &Sender<Message<C>>)
where L: Log<C> {

    // TODO consider how to ack the client properly, once the majority vote is received.
    // TODO One way to do this is to note the LogPosition of the append, then have
    // TODO something watch the log, and once the LogPosition is superseded, ack the
    // TODO client. If the log position invalidates the append (e.g. due to term
    // TODO changing), then we can NACK the client.  And if nothing happens, we can
    // TODO just forget about the request, or NACK with a timeout.
    //
    // TODO We don't just want to ACK once committed.  The application state machine itself
    // TODO might produce a meaningful output upon apply, and the client would want to see that.
    // TODO Considering that, I don't like the layering of this app.  I should have raft govern
    // TODO the log only, and handle the application state machine and the client state machine
    // TODO at a layer above.  That would make it easier to handle responding to the client properly.
    // TODO And I don't think it would over-complicate log-compaction.

    match state.my_role() {
        Role::Leader(_) => {
            let last = state.log().last();
            let next = last.0.next();
            let entry = LogEntry {
                position: LogPosition(next, state.term()),
                payload: Payload::ChangeState(command),
            };
            state.log_mut().append_if(vec![entry], last).expect("leader should be able to append command to log");

            // we automatically ack this entry
            let myself = state.myself();
            state.leader_state_mut().set_next_index_for(myself, next.next());
            state.leader_state_mut().set_match_index_for(myself, next);

            broadcast_log(state, sender);

            // Incorrect, obviously, because we're acking something that hasn't yet been committed.
            responder.respond(ClientResponse::Received(state.log().last()));
        },
        Role::Follower(extra_follower_state) => {
            match extra_follower_state.leader() {
                Some(leader) => responder.respond(ClientResponse::Redirect(leader)),
                None => responder.respond(ClientResponse::UnknownLeader),
            }
        },
        Role::Candidate(_) => {
            responder.respond(ClientResponse::UnknownLeader);
        },
    }
}
