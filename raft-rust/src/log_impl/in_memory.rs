use std::collections::HashMap;

use crate::raft::log::Log;
use crate::raft::log::LogEntry;
use crate::raft::log::LogPosition;
use crate::raft::log::Payload;
use crate::raft::log::WrongPreviousEntry;
use crate::raft::membership::NodeId;
use crate::raft::state::Index;
use crate::raft::state::Term;

pub struct InMemoryLog<Cmd> {
    data: HashMap<Index, LogEntry<Cmd>>,
    tip: Index,
}
impl<Cmd> InMemoryLog<Cmd> {
    pub fn new() -> Self {
        let mut data = HashMap::new();
        let seed = LogEntry {
            position: LogPosition::default(),
            payload: Payload::Noop(NodeId::default()),
        };
        data.insert(Index::default(), seed);
        Self {
            data: data,
            tip: Index::default(),
        }
    }
}
impl<Cmd> Log<Cmd> for InMemoryLog<Cmd>
where Cmd: Clone {

    fn append_if(&mut self, entries: Vec<LogEntry<Cmd>>, prev: LogPosition) -> Result<LogPosition, WrongPreviousEntry> {

        let entry_at_prev = self.data.get(&prev.0);

        let matches = match entry_at_prev {
            None => false,
            Some(entry) => entry.position.1 == prev.1, // do the terms match?
        };

        if !matches {
            return Err(WrongPreviousEntry);
        }

        // first clear out everything starting at the log position
        self.data.retain(|idx, _| idx <= &prev.0 );

        // we know the latest position is `prev` at this point because of the above checks/clear
        let mut latest_position = prev;

        // TODO check to make sure entries are correct before inserting them
        for entry in entries {
            latest_position = entry.position;
            self.data.insert(entry.position.0.clone(), entry);
        }

        self.tip = latest_position.0;

        Ok(latest_position)
    }

    fn scan(&self, from: Index, to: Option<Index>, closure: &mut impl FnMut(LogEntry<Cmd>)) {
        let mut curr = from;
        let last = self.last().0;
        while curr <= last && (to.is_none() || (to.is_some() && curr <= to.unwrap())) {
            let entry = self.data.get(&curr).unwrap().clone();
            closure(entry);
            curr = curr.next();
        }
    }

    fn last(&self) -> LogPosition {
        self.data.get(&self.tip).unwrap().position
    }

    fn term_for(&self, index: Index) -> Term {
        self.data.get(&index).unwrap().position.1
    }

    fn get_term_info(&self) -> (Term, Option<NodeId>) {
        (Term::default(), None)
    }

    fn set_term_info(&mut self, _term: Term, _member: Option<NodeId>) {
        ; // we don't save state in this impl
    }
}