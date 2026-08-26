#![forbid(unsafe_code)]

//! The Journey: one line of execution, and the record of what happened on it.
//!
//! Merged 2026-08-26 from two implementations. This repository had the
//! structure — `JourneyEntry` and the link to the Journey before — and the root
//! repository had the vocabulary that ADR-0013 settled. Neither was wholesale
//! right.

use xmip_core::{ExecutionId, JourneyId, MessageId, StreamId};

/// The operational state of a Journey.
///
/// Three of these are terminal — `Completed`, `Failed` and `Dismissed` — and
/// the distinction between the last two is why `Dismissed` exists. A Journey
/// that failed hit something it could not get past. A Journey that was
/// dismissed was stopped by a decision: an operator, or a Process recognising
/// a duplicate. Collapsing them means every deliberate stop reads as an error
/// and every failure count is inflated by correct behaviour.
///
/// Replaces the v1.0 set this repository carried — `Created`, `Running`,
/// `Paused`, `Dead` — which `runtime-model.md` section 23 retired. `Created`
/// has no successor on purpose: a Journey exists only after Validation, so
/// there is nothing for it to be created in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyState {
    Active,
    Waiting,
    Suspended,
    Recovering,
    Completed,
    Failed,
    Dismissed,
}

impl JourneyState {
    /// Whether the Journey has stopped for good.
    ///
    /// `Suspended` is not terminal — an operator suspended it and an operator
    /// can resume it. Nor is `Recovering`: a Retry is in flight.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Dismissed)
    }

    /// Whether the Journey stopped without completing its work.
    ///
    /// `Failed` and `Dismissed` both answer yes, which is what most reporting
    /// wants. Use the variant itself where the difference between a fault and
    /// a decision matters, because that difference is why they are separate.
    #[must_use]
    pub fn is_incomplete(self) -> bool {
        matches!(self, Self::Failed | Self::Dismissed)
    }
}

/// One thing that happened, in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyEntry {
    pub execution_id: ExecutionId,
    pub message_id: MessageId,
    pub action: String,
    pub outcome: String,
    pub timestamp_unix_nanos: i128,
}

/// One Message generation this Journey has held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JourneyMessageRef {
    pub message_id: MessageId,
    pub stream_id: StreamId,
}

/// One line of execution. Not a tree — ADR-0013 clause 5.
///
/// A Journey accumulates: entries, Message generations, state transitions. Its
/// historical record is appended to and never rewritten. The Streams it refers
/// to never change at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Journey {
    pub journey_id: JourneyId,

    pub state: JourneyState,

    /// The Journey this one came from, if any.
    ///
    /// A Process that splits a Message or publishes back into Xmip starts new
    /// Journeys, and each names the one that caused it. ADR-0013 clause 4b.
    ///
    /// *Previous*, not *parent*: parent implies containment and reads as a
    /// contradiction of clause 5. Each Journey is a line; the relationships
    /// between them form a chain. Several may share one previous Journey, which
    /// is what happens when a Publication matches several Subscriptions.
    ///
    /// Known limit: this names the causing Journey, not the causing event. A
    /// Journey that publishes twice leaves a successor able to say which
    /// Journey started it and not which publication within it.
    pub previous_journey_id: Option<JourneyId>,

    pub current_xmip_process: Option<String>,

    pub entries: Vec<JourneyEntry>,

    pub messages: Vec<JourneyMessageRef>,
}

impl Journey {
    /// A Journey with no history and no predecessor.
    #[must_use]
    pub fn new(journey_id: JourneyId) -> Self {
        Self {
            journey_id,
            state: JourneyState::Active,
            previous_journey_id: None,
            current_xmip_process: None,
            entries: Vec::new(),
            messages: Vec::new(),
        }
    }

    /// A Journey caused by another.
    #[must_use]
    pub fn following(journey_id: JourneyId, previous: JourneyId) -> Self {
        Self {
            previous_journey_id: Some(previous),
            ..Self::new(journey_id)
        }
    }

    /// Append what happened and move to the state it left the Journey in.
    #[must_use]
    pub fn append(mut self, entry: JourneyEntry, state: JourneyState) -> Self {
        self.entries.push(entry);
        self.state = state;
        self
    }

    /// Record a Message generation this Journey now holds.
    #[must_use]
    pub fn holding(mut self, message: JourneyMessageRef) -> Self {
        self.messages.push(message);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_journey_is_active_and_has_no_predecessor() {
        let journey = Journey::new(JourneyId::new(1));

        assert_eq!(journey.state, JourneyState::Active);
        assert!(journey.previous_journey_id.is_none());
        assert!(!journey.state.is_terminal());
    }

    #[test]
    fn a_following_journey_names_the_one_before_it() {
        let first = Journey::new(JourneyId::new(1));
        let second = Journey::following(JourneyId::new(2), first.journey_id);

        assert_eq!(second.previous_journey_id, Some(first.journey_id));
    }

    #[test]
    fn several_journeys_may_share_one_previous() {
        // One Publication matching three Subscriptions. Each Journey is a
        // line; the relationships between them are not.
        let cause = JourneyId::new(1);
        let journeys: Vec<Journey> = (2..5)
            .map(|n| Journey::following(JourneyId::new(n), cause))
            .collect();

        assert!(journeys.iter().all(|j| j.previous_journey_id == Some(cause)));
    }

    #[test]
    fn dismissed_is_terminal_but_is_not_a_failure() {
        assert!(JourneyState::Dismissed.is_terminal());
        assert!(JourneyState::Dismissed.is_incomplete());
        assert_ne!(JourneyState::Dismissed, JourneyState::Failed);
    }

    #[test]
    fn suspended_and_recovering_are_not_terminal() {
        assert!(!JourneyState::Suspended.is_terminal());
        assert!(!JourneyState::Recovering.is_terminal());
    }
}
