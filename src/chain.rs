//! What bounds a publication chain.
//!
//! A Process may publish back into Xmip. A Subscription may start a Process. So
//! a Process that publishes a Message matching a Subscription that starts the
//! same Process is a loop, and nothing about either half is wrong on its own.
//!
//! This is the classic way an integration platform takes itself down, and it
//! does it at three in the morning with a message that looked ordinary. ADR-0026
//! decides the answer: a depth limit on the chain, enforced where the chain is
//! built, so a runtime cannot extend one past its ceiling by any path.
//!
//! **The limit is not cycle detection**, and the difference matters when reading
//! a refusal. A depth limit cannot tell a loop from a long legitimate chain; it
//! only knows that this one went further than the estate allows. That is why
//! [`ChainRefused`] names the Subscription and the Process at the point of
//! refusal — an operator reading configuration files at three in the morning
//! needs the pair that formed the loop, not a number.

use core::fmt;

use xmip_core::JourneyId;

/// How deep a publication chain may run.
///
/// Depth counts links, not Journeys: a Journey that nothing caused is at depth
/// zero, the one it publishes into is at one, and a limit of `n` permits `n`
/// links and refuses the one after.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChainLimit(u32);

impl ChainLimit {
    /// The ceiling a node uses when its configuration names none.
    ///
    /// Thirty-two, and the number is a starting point rather than a finding.
    /// ADR-0026 says so plainly: no estate has run long enough to measure what
    /// a legitimate chain reaches, and a default chosen before there is traffic
    /// is a guess whatever it is set to. It is deliberately far above any chain
    /// anyone has described and far below the depth at which a loop becomes
    /// expensive.
    pub const DEFAULT: Self = Self(32);

    /// A ceiling of this many links.
    #[must_use]
    pub const fn new(links: u32) -> Self {
        Self(links)
    }

    /// The ceiling, as a number.
    #[must_use]
    pub const fn links(self) -> u32 {
        self.0
    }

    /// Whether a chain already this deep may gain one more link.
    #[must_use]
    pub const fn permits(self, depth: u32) -> bool {
        depth < self.0
    }
}

impl Default for ChainLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Why one Journey caused another.
///
/// The Subscription that matched, and the Xmip Process it started where it
/// started one. Both halves are what an operator needs to see a loop, and
/// neither is recoverable from a Journey id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainCause {
    pub subscription_id: String,
    pub xmip_process: Option<String>,
}

impl ChainCause {
    /// A Subscription that matched and started nothing but a delivery.
    #[must_use]
    pub fn subscription(subscription_id: impl Into<String>) -> Self {
        Self {
            subscription_id: subscription_id.into(),
            xmip_process: None,
        }
    }

    /// A Subscription that matched and started an Xmip Process.
    #[must_use]
    pub fn process(subscription_id: impl Into<String>, xmip_process: impl Into<String>) -> Self {
        Self {
            subscription_id: subscription_id.into(),
            xmip_process: Some(xmip_process.into()),
        }
    }
}

impl fmt::Display for ChainCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.xmip_process {
            Some(process) => {
                write!(
                    formatter,
                    "Subscription '{}' starting Xmip Process '{}'",
                    self.subscription_id, process
                )
            }
            None => write!(formatter, "Subscription '{}'", self.subscription_id),
        }
    }
}

/// A publication chain that reached its limit, and what was at the end of it.
///
/// Returned rather than thrown away: the Message is not lost by refusing to
/// extend the chain, and what happens to it is a disposition like any other.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainRefused {
    /// The ceiling in force when the chain was refused.
    pub limit: ChainLimit,

    /// The depth already reached. Equal to the limit, by construction.
    pub depth: u32,

    /// The Journey that would have caused the refused one.
    pub previous_journey_id: JourneyId,

    /// The Subscription and Process that would have formed the next link.
    pub cause: ChainCause,
}

impl fmt::Display for ChainRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "publication chain refused at depth {} of {}: {} would extend Journey {:?}",
            self.depth,
            self.limit.links(),
            self.cause,
            self.previous_journey_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_limit_permits_a_chain_below_it() {
        assert!(ChainLimit::DEFAULT.permits(0));
        assert!(ChainLimit::DEFAULT.permits(31));
    }

    #[test]
    fn a_limit_refuses_the_link_after_its_last() {
        assert!(!ChainLimit::DEFAULT.permits(32));
        assert!(!ChainLimit::DEFAULT.permits(33));
    }

    #[test]
    fn a_limit_of_zero_permits_no_link_at_all() {
        // Not a degenerate case: it is how a node says a Process may not
        // publish back into Xmip at all.
        assert!(!ChainLimit::new(0).permits(0));
    }

    #[test]
    fn a_cause_says_the_subscription_and_the_process() {
        let cause = ChainCause::process("billing", "Approval");

        assert_eq!(
            cause.to_string(),
            "Subscription 'billing' starting Xmip Process 'Approval'"
        );
    }

    #[test]
    fn a_cause_with_no_process_says_only_the_subscription() {
        let cause = ChainCause::subscription("billing");

        assert_eq!(cause.to_string(), "Subscription 'billing'");
        assert!(cause.xmip_process.is_none());
    }

    #[test]
    fn a_refusal_names_the_pair_that_formed_the_loop() {
        let refused = ChainRefused {
            limit: ChainLimit::new(4),
            depth: 4,
            previous_journey_id: JourneyId::new(7),
            cause: ChainCause::process("billing", "Approval"),
        };

        let said = refused.to_string();

        assert!(said.contains("depth 4 of 4"));
        assert!(said.contains("Subscription 'billing'"));
        assert!(said.contains("Xmip Process 'Approval'"));
    }
}
