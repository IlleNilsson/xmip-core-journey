#![forbid(unsafe_code)]

use xmip_core::{ExecutionId, JourneyId, MessageId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyState {
    Created,
    Running,
    Paused,
    Waiting,
    Dead,
    Completed,
    Dismissed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyEntry {
    pub execution_id: ExecutionId,
    pub message_id: MessageId,
    pub action: String,
    pub outcome: String,
    pub timestamp_unix_nanos: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Journey {
    pub journey_id: JourneyId,
    pub state: JourneyState,
    pub entries: Vec<JourneyEntry>,
    pub parent_journey_id: Option<JourneyId>,
}

impl Journey {
    pub fn new(journey_id: JourneyId) -> Self {
        Self {
            journey_id,
            state: JourneyState::Created,
            entries: Vec::new(),
            parent_journey_id: None,
        }
    }

    pub fn append(mut self, entry: JourneyEntry, state: JourneyState) -> Self {
        self.entries.push(entry);
        self.state = state;
        self
    }
}
