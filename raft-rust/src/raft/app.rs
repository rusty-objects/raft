
/// Trait implemented by the underlying state machine (e.g. the application's business logic).
/// Raft is agnostic of what the underlying application actually does.
pub trait ApplicationStateMachine<Cmd> {
    type Snapshot;
    type Output;

    /// Apply a change to the underlying state machine.  This occurs after the results
    /// been committed to raft, so the transition is considered committed.  However the
    /// business logic associated with the state machine might accept, reject, or do something
    /// else with the agreed upon input (e.g. all the raft members might have agreed to
    /// commit the "divide by zero" command into the log, but when it comes time to apply it,
    /// it's illegal).  The output here is used by the leader to send back to the original
    /// client once the leader commits it to the log (while the client is still waiting).
    /// Everyone else is free to ignore the response as they're applying transitions to
    /// their log.
    fn apply(&mut self, transition: Cmd) -> Self::Output;

    /// Create a snapshot of the underlying state.  Used for log compaction.
    fn snapshot(&mut self) -> Self::Snapshot;

    /// This isn't part of raft and isn't critical to correctness.  This is just
    /// a mechanism for advertising who the leader is for logging, debugging, etc.
    /// It's in the application state machine in case there's an application specific
    /// means to report the leader.
    fn i_became_leader(&self);
}
