#![forbid(unsafe_code)]

//! The Journey: one line of execution, and the record of what happened on it.
//!
//! Merged 2026-08-26 from two implementations. This repository had the
//! structure — `JourneyEntry` and the link to the Journey before — and the root
//! repository had the vocabulary that ADR-0013 settled. Neither was wholesale
//! right.

pub mod chain;

pub use chain::{ChainCause, ChainLimit, ChainRefused};

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

    /// Why this Journey exists, where something caused it.
    ///
    /// `previous_journey_id` says which Journey; this says which Subscription
    /// matched and which Xmip Process it started. A Journey that nothing caused
    /// has neither. ADR-0026.
    pub cause: Option<ChainCause>,

    /// How many links back to a Journey that nothing caused.
    ///
    /// Zero for a Journey that arrived from outside Xmip. One more than its
    /// predecessor for every Journey a Publication caused, which is what
    /// [`ChainLimit`] is compared against before the link is made. ADR-0026.
    pub depth: u32,

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
            cause: None,
            depth: 0,
            current_xmip_process: None,
            entries: Vec::new(),
            messages: Vec::new(),
        }
    }

    /// A Journey caused by another, or the refusal that says why not.
    ///
    /// **This is the only way a publication chain grows**, which is what makes
    /// the ceiling worth anything: a runtime cannot extend a chain past its
    /// limit by taking a different path, because there is no other path. The
    /// bound ships with the chain rather than after it — ADR-0026, and the lean
    /// recorded against open problem 13.
    ///
    /// The refusal is a value and not a panic. The Message that would have
    /// started the next Journey is not lost by declining to start it; what
    /// happens to it is a disposition, decided by the caller.
    ///
    /// # Errors
    ///
    /// [`ChainRefused`] when `previous` is already at the limit, naming the
    /// Subscription and the Xmip Process that would have formed the next link.
    pub fn following(
        journey_id: JourneyId,
        previous: &Self,
        cause: ChainCause,
        limit: ChainLimit,
    ) -> Result<Self, ChainRefused> {
        if !limit.permits(previous.depth) {
            return Err(ChainRefused {
                limit,
                depth: previous.depth,
                previous_journey_id: previous.journey_id,
                cause,
            });
        }

        Ok(Self {
            previous_journey_id: Some(previous.journey_id),
            cause: Some(cause),
            depth: previous.depth + 1,
            ..Self::new(journey_id)
        })
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

    fn caused_by(previous: &Journey, id: u128) -> Journey {
        Journey::following(
            JourneyId::new(id),
            previous,
            ChainCause::subscription("billing"),
            ChainLimit::DEFAULT,
        )
        .expect("the default limit permits this depth")
    }

    #[test]
    fn a_following_journey_names_the_one_before_it() {
        let first = Journey::new(JourneyId::new(1));
        let second = caused_by(&first, 2);

        assert_eq!(second.previous_journey_id, Some(first.journey_id));
        assert_eq!(second.cause, Some(ChainCause::subscription("billing")));
    }

    #[test]
    fn several_journeys_may_share_one_previous() {
        // One Publication matching three Subscriptions. Each Journey is a
        // line; the relationships between them are not.
        let first = Journey::new(JourneyId::new(1));
        let journeys: Vec<Journey> = (2..5).map(|n| caused_by(&first, n)).collect();

        assert!(
            journeys
                .iter()
                .all(|j| j.previous_journey_id == Some(first.journey_id))
        );

        // Siblings, not a deepening chain. Three matches of one Publication
        // are all one link from the Journey that published.
        assert!(journeys.iter().all(|j| j.depth == 1));
    }

    #[test]
    fn a_journey_that_arrived_from_outside_is_at_depth_zero() {
        let journey = Journey::new(JourneyId::new(1));

        assert_eq!(journey.depth, 0);
        assert!(journey.cause.is_none());
    }

    #[test]
    fn each_link_deepens_the_chain_by_one() {
        let mut journey = Journey::new(JourneyId::new(1));

        for id in 2..12 {
            journey = caused_by(&journey, id);
        }

        assert_eq!(journey.depth, 10);
    }

    #[test]
    fn the_chain_is_refused_at_the_limit_and_says_what_formed_it() {
        let limit = ChainLimit::new(2);
        let mut journey = Journey::new(JourneyId::new(1));

        for id in 2..4 {
            journey = Journey::following(
                JourneyId::new(id),
                &journey,
                ChainCause::process("billing", "Approval"),
                limit,
            )
            .expect("within the limit");
        }

        let refused = Journey::following(
            JourneyId::new(4),
            &journey,
            ChainCause::process("billing", "Approval"),
            limit,
        )
        .expect_err("the third link is past a limit of two");

        assert_eq!(refused.depth, 2);
        assert_eq!(refused.limit, limit);
        assert_eq!(refused.previous_journey_id, journey.journey_id);
        assert_eq!(refused.cause.subscription_id, "billing");
        assert_eq!(refused.cause.xmip_process.as_deref(), Some("Approval"));
    }

    #[test]
    fn a_refused_link_leaves_the_previous_journey_untouched() {
        // Refusing to extend a chain is not a failure of the Journey that
        // published. It goes on being whatever it was.
        let journey = Journey::new(JourneyId::new(1));
        let before = journey.clone();

        let refused = Journey::following(
            JourneyId::new(2),
            &journey,
            ChainCause::subscription("billing"),
            ChainLimit::new(0),
        );

        assert!(refused.is_err());
        assert_eq!(journey, before);
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
