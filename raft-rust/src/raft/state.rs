use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;

use rand::Rng;

use crate::raft::app::ApplicationStateMachine;
use crate::raft::log::Log;
use crate::raft::log::LogEntry;
use crate::raft::log::Payload;
use crate::raft::membership::Membership;
use crate::raft::membership::NodeId;

pub const HEARTBEAT_TIMEOUT: Duration = ms_to_duration(1000);
const fn ms_to_duration(amt: u64) -> Duration {
    Duration::from_millis(amt)
}
pub const ELECTION_TIMEOUT: u64 = 10000;
fn election_timeout() -> Duration {
    let jitter = rand::thread_rng().gen_range(0, ELECTION_TIMEOUT/2);
    Duration::from_millis(ELECTION_TIMEOUT + jitter)
}

/// A term, as defined by the raft protocol.  Represents a phase, generation, epoch, etc. in which
/// a single leader is operating and attempting to garner consensus amongst its followers.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug, Serialize, Deserialize)]
pub struct Term(pub u64);
impl Term {
    pub fn next(&self) -> Term {
        Term(self.0+1)
    }
}

/// Absolute position in the log
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug, Hash, Serialize, Deserialize)]
pub struct Index(pub u64);
impl Index {
    pub fn next(&self) -> Index {
        Index(self.0+1)
    }

    pub fn prev(&self) -> Index {
        Index(self.0-1)
    }
}

/// An enum capturing our current self-perceived role
pub enum Role {
    Leader(ExtraLeaderState),
    Follower(ExtraFollowerState),
    Candidate(ExtraCandidateState),
}

/// State for the raft leader
pub struct ExtraLeaderState {
    term: Term,
    next_index: HashMap<NodeId, Index>,
    match_index: HashMap<NodeId, Index>,
    next_heartbeat_due: Instant,
}
impl ExtraLeaderState {

    /// What is the greatest index such that at least a majority of members have accepted
    /// an index >= this value.  Note that this is similar but not quite equal to median,
    /// as the definition of median when there are an even number of participants is the
    /// average of the value surrounding the midpoint.  Here we want a value that appears
    /// in the set, and we want it to be the value just at the first member not in the bottom
    /// half (e.g. 3rd of 5 members, or 4th of 6 members)
    pub fn majority_match_index(&self) -> Index {
        let mut values: Vec<Index> = self.match_index.values().map(|i| i.clone()).collect();
        values.sort();
        let idx = values.len()/2 + 1;
        values[idx]
    }

    pub fn is_past_heartbeat_timeout(&self) -> bool {
        Instant::now() >= self.next_heartbeat_due
    }

    pub fn reset_heartbeat_timeout(&mut self) {
        self.next_heartbeat_due = Instant::now() + HEARTBEAT_TIMEOUT;
    }

    pub fn set_next_index_for(&mut self, member: NodeId, index: Index) {
        self.next_index.insert(member, index);
    }

    pub fn set_match_index_for(&mut self, member: NodeId, index: Index) {
        self.match_index.insert(member, index);
    }

    pub fn next_index_for(&self, member: NodeId) -> Index {
        self.next_index.get(&member).unwrap().clone()
    }

    /// Sections 5.3 and 5.4: The leader's commit index is the greatest
    /// entry index that a majority of members agree on, as long as its
    /// in the current term.
    pub fn commit_index<Cmd>(&self, log: &impl Log<Cmd>) -> Option<Index> {
        let possible_commit_index = self.majority_match_index();
        if log.term_for(possible_commit_index) == self.term {
            Some(possible_commit_index)
        } else {
            None
        }
    }

}

/// State for raft candidates
pub struct ExtraCandidateState {
    yes_votes: HashSet<NodeId>,
}
impl ExtraCandidateState {
    pub fn new() -> Self {
        Self {
            yes_votes: HashSet::new(),
        }
    }

    pub fn yes_votes(&self) -> &HashSet<NodeId> {
        &self.yes_votes
    }

    pub fn insert_yes_vote(&mut self, voter: NodeId) {
        self.yes_votes.insert(voter);
    }
}

pub struct ExtraFollowerState {
    /// Tracks the current leader, if known.  Sometimes this will be unknown, for example if we lose an election.
    leader: Option<NodeId>,
}
impl ExtraFollowerState {
    pub fn new() -> Self {
        Self {
            leader: None,
        }
    }

    pub fn update_leader(&mut self, leader: NodeId) {
        self.leader = Some(leader);
    }

    pub fn leader(&self) -> Option<NodeId> {
        self.leader
    }
}

pub struct RaftState<Log, Application> {
    myself: NodeId,
    my_role: Role,
    log: Log,
    application: Application,

    term: Term,
    voted_for: Option<NodeId>,
    commit_index: Index,
    last_applied: Index,
    membership: Membership,
    election_expiry: Instant,
}
impl<L, A> RaftState<L, A> {

    pub fn new<Cmd>(myself: NodeId, log: L, application: A, membership: Membership) -> Self
    where L: Log<Cmd>, A: ApplicationStateMachine<Cmd> {
        let (term, voted_for) = log.get_term_info();
        Self {
            myself,
            my_role: Role::Follower(ExtraFollowerState::new()),
            log,
            application,

            term,
            voted_for,
            commit_index: Index::default(), // TODO when we have snapshotting, this gets seeded with it
            last_applied: Index::default(), // TODO when we have snapshotting, this gets seeded with it
            membership,
            election_expiry: Instant::now() + election_timeout(),
        }
    }

    pub fn myself(&self) -> NodeId {
        self.myself
    }

    pub fn my_role(&self) -> &Role {
        &self.my_role
    }

    pub fn log(&self) -> &L {
        &self.log
    }

    pub fn log_mut(&mut self) -> &mut L {
        &mut self.log
    }

    pub fn application(&mut self) -> &mut A {
        &mut self.application
    }

    pub fn term(&self) -> Term {
        self.term
    }

    pub fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    /// Sets the commit index if the supplied candidate has moved forward.
    pub fn maybe_set_commit_index(&mut self, candidate_index: Index) {
        self.commit_index = self.commit_index.max(candidate_index);
    }

    /// Retrieves the current commit_index
    pub fn commit_index(&mut self) -> Index {
        self.commit_index
    }

    pub fn is_past_election_timeout(&self) -> bool {
        Instant::now() >= self.election_expiry
    }

    pub fn reset_election_timeout(&mut self) {
        self.election_expiry = Instant::now() + election_timeout();
    }

    pub fn time_until_next_interesting_event(&self) -> Duration {
        let now = Instant::now();
        match &self.my_role {
            Role::Leader(leader_state) => leader_state.next_heartbeat_due - now,
            Role::Follower(_) => self.election_expiry - now,
            Role::Candidate(_) => self.election_expiry - now,
        }
    }

    pub fn membership(&self) -> &Membership {
        &self.membership
    }

    pub fn replace_membership(&mut self, new_membership: Membership) {
        self.membership = new_membership
    }

    pub fn vote_for<Cmd>(&mut self, voted_for: NodeId) where L: Log<Cmd> {
        assert!(self.voted_for.is_none() || Some(&voted_for) == self.voted_for.as_ref(), "invariant violation: changing vote for term");
        self.voted_for = Some(voted_for);
        self.log.set_term_info(self.term, self.voted_for.clone());
    }

    pub fn leader_state_mut(&mut self) -> &mut ExtraLeaderState {
        match &mut self.my_role {
            Role::Leader(leader_state) => leader_state,
            Role::Candidate(_) => panic!("invariant violation: access leader_state while Candidate"),
            Role::Follower(_) => panic!("invariant violation: access leader_state while Follower"),
        }
    }

    pub fn follower_state_mut(&mut self) -> &mut ExtraFollowerState {
        match &mut self.my_role {
            Role::Leader(_) => panic!("invariant violation: access follower_state while Leader"),
            Role::Candidate(_) => panic!("invariant violation: access follower_state while Candidate"),
            Role::Follower(follower_state) => follower_state,
        }
    }

    pub fn leader_state(&self) -> &ExtraLeaderState {
        match &self.my_role {
            Role::Leader(leader_state) => leader_state,
            Role::Candidate(_) => panic!("invariant violation: access leader_state while Candidate"),
            Role::Follower(_) => panic!("invariant violation: access leader_state while Follower"),
        }
    }

    pub fn is_candidate(&self) -> bool {
        match self.my_role {
            Role::Leader(_) => false,
            Role::Candidate(_) => true,
            Role::Follower(_) => false,
        }
    }

    pub fn candidate_state_mut(&mut self) -> &mut ExtraCandidateState {
        match &mut self.my_role {
            Role::Leader(_) => panic!("invariant violation: access candidate_state while Leader"),
            Role::Candidate(candidate_state) => candidate_state,
            Role::Follower(_) => panic!("invariant violation: access candidate_state while Follower"),
        }
    }

    /// Decides if we've won the election.  Can only be invoked from the candidate state.
    pub fn candidate_has_won_election(&self) -> bool {
        match &self.my_role {
            Role::Leader(_) => panic!("invariant violation: access candidate_state while Leader"),
            Role::Candidate(candidate_state) => self.membership.is_majority(&candidate_state.yes_votes()),
            Role::Follower(_) => panic!("invariant violation: access candidate_state while Follower"),
        }
    }

    // There are a couple ways to turn into a follower.
    //
    // Section 5.1 requires that the receipt of any message with term > our term resets our
    // term to the higher term and resets our status to follower.  This is critical, for example
    // so that if someone who doesn't have an updated log gets isolated in the network and increments
    // its term a bunch trying to win an election, when it finally rejoins it'll force all members
    // to update their term so that the most updated members can win the election at the new term
    // and the formerly isolated member can rejoin.  One related optimization here for members that
    // are isolated from the cluster: if numerous elections timeout pass with no apparent leader,
    // backoff and increase the election timeout.  Probably doesn't matter with a 64 bit term, but
    // it'll keep isolated members from obsessively increasing their term during a multi-hour network
    // partition.
    //
    // Leader timeout?
    pub fn turn_into_follower<Cmd>(&mut self, new_term: Term)
    where L: Log<Cmd> {
        assert!(self.term < new_term || self.term == new_term && self.is_candidate(), "invariant violation: term must go forwards");
        self.term = new_term;
        self.voted_for = None;
        self.log.set_term_info(new_term, None);

        self.my_role = Role::Follower(ExtraFollowerState::new());
    }

    /// When we turn into a leader, we have to create some extra state in order
    /// to track where everyone is in terms of replication and commits.
    pub fn turn_into_leader<Cmd>(&mut self)
    where L: Log<Cmd>, A: ApplicationStateMachine<Cmd> {
        assert!(self.is_candidate(), "invariant violation: turn_into_leader while not candidate");

        let members = self.membership.as_set();

        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();

        let next = self.log().last().0.next();
        for member in members {
            next_index.insert(member.clone(), next);
            match_index.insert(member.clone(), Index::default());
        }

        let leader_state = ExtraLeaderState {
            term: self.term,
            next_index,
            match_index,
            next_heartbeat_due: Instant::now() + HEARTBEAT_TIMEOUT,
        };

        self.my_role = Role::Leader(leader_state);
        self.application.i_became_leader();
    }

    /// When we turn into a candidate, we increment the term and vote for ourselves.
    pub fn turn_into_candidate<Cmd>(&mut self) where L: Log<Cmd> {
        self.term = self.term.next();
        self.voted_for = Some(self.myself.clone());
        self.my_role = Role::Candidate(ExtraCandidateState::new());
        let voter = self.myself.clone();
        self.candidate_state_mut().insert_yes_vote(voter);
        self.log.set_term_info(self.term, self.voted_for.clone());
    }

    /// Applies any commands in the range (last_applied, commit_index] to the application state machine,
    /// and updates last_applied appropriately.
   pub fn apply_commands<Cmd>(&mut self)
    where L: Log<Cmd>, A: ApplicationStateMachine<Cmd> {
        let commit = self.commit_index();
        let app = &mut self.application;
        self.log.scan(self.last_applied.next(), Some(commit), &mut |entry: LogEntry<Cmd>| {

            // apply the record
            match entry.payload {
                Payload::Noop(_) => (),
                Payload::ChangeMembership(_) => (),
                Payload::ChangeState(command) => {Some(app.apply(command));},
            };

        });
        self.last_applied = commit;
    }
}

#[cfg(test)]
pub mod test {

    // TODO

}
