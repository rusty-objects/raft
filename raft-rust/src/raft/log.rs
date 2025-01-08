use crate::raft::membership::Membership;
use crate::raft::state::Index;
use crate::raft::state::Term;
use std::fmt;
use crate::raft::membership::NodeId;

// The types of messages that can be appended in the log
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Payload<Cmd> {
    /// A noop is a message from a leader that creates a log entry in the leader's term with
    /// no side effects.  These are necessary immediately after a new Term for correctness,
    /// otherwise it's possible for entries to be committed by other leaders during untimely
    /// crashes.  See section 5.4.2 and Figure 8 of the raft paper.
    ///
    /// The noop contains the end point of the leader who sent it, just to make that information
    /// durable for the term.
    ///
    /// Note that every log starts at index:0, term:0 with a Noop with a dummy node-id.  So every
    /// log implicitly agrees on this initial entry.
    Noop(NodeId),

    /// A request to change the cluster membership.  See section 6 of the raft paper.
    ChangeMembership(Membership),

    /// A state change.  The actual state depends on the problem domain that raft is being applied to.
    /// The raft algorithm itself is agnostic of what the application-level state machine actually does.
    ChangeState(Cmd),
}
impl<Cmd: fmt::Debug> fmt::Debug for Payload<Cmd> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Payload::Noop(leader) => write!(f, "Payload::Noop({:?})", leader),
            Payload::ChangeMembership(membership) => write!(f, "Payload::ChangeMembership({:?})", membership),
            Payload::ChangeState(cmd) => write!(f, "Payload::ChangeState({:?})", cmd),
        }
    }
}

// A log entry.  This is ultimately the entire point of raft: nodes are building identical logs,
/// and raft is a mechanism for agreeing on the set of these.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry<Cmd> {
    pub position: LogPosition,
    pub payload: Payload<Cmd>,
}
impl<Cmd> LogEntry<Cmd> {
    pub fn new(position: LogPosition, payload: Payload<Cmd>) -> Self {
        Self { position, payload }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct LogPosition(pub Index, pub Term);
impl LogPosition {

    // the definition of a log being up to date is defined in section 5.4.1:
    // if our term is higher than the other's, or if the terms are the same
    // but our index is higher, then we are up to date
    pub fn is_up_to_date(&self, compared_to: LogPosition) -> bool {
        self.1 > compared_to.1 || (self.1 == compared_to.1 && self.0 >= compared_to.0)
    }
}

/// Log impl, abstract to allow in-memory, on-disk, etc.
pub trait Log<Cmd> {
    /// Add to the log if the previous entry has the supplied index and term
    /// Returns the new latest LogPosition if successful, or an error otherwise.
    fn append_if(&mut self, entries: Vec<LogEntry<Cmd>>, prev: LogPosition) -> Result<LogPosition, WrongPreviousEntry>;

    /// `Scan` allows for reading from a particular index through to the supplied
    /// `to` value, or to the end if missing.
    fn scan(&self, from: Index, to: Option<Index>, closure: &mut impl FnMut(LogEntry<Cmd>));

    /// What was the last entry written?
    fn last(&self) -> LogPosition;

    /// What was the last entry written?
    fn term_for(&self, index: Index) -> Term;

    /// Return the latest term and who we've voted for in that term, if any
    fn get_term_info(&self) -> (Term, Option<NodeId>);

    /// Durably record the latest term info and who we've voted for in that term
    fn set_term_info(&mut self, term: Term, member: Option<NodeId>);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WrongPreviousEntry;

#[cfg(test)]
pub mod test {
    use crate::raft::state::Index;
    use crate::raft::state::Term;
    use super::*;

    #[test]
    pub fn is_up_to_date() {
        let pos1 = LogPosition(Index(0), Term(0));
        let pos2 = LogPosition(Index(1), Term(0));
        let pos3 = LogPosition(Index(0), Term(3));

        assert!(pos1.is_up_to_date(pos1));

        assert!(!pos1.is_up_to_date(pos2));
        assert!(pos2.is_up_to_date(pos1));

        assert!(!pos1.is_up_to_date(pos3));
        assert!(pos3.is_up_to_date(pos1));

        assert!(!pos2.is_up_to_date(pos3));
        assert!(pos3.is_up_to_date(pos2));
    }
}