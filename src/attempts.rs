//! Where a Journey's send stands at its Send Port: its retry history, kept
//! with the Journey in the Ledger (ADR-0013: *A Journey accumulates a story:
//! execution history, … state transitions, retry history*).

/// The active Send Location of a Journey's Send Port and how often it has
/// been tried: written with every hand-on of a send, so a retry's count
/// survives a restart and another node taking the Journey up goes on from
/// it (`runtime-model.md` section 10: *Retries are on the active Send
/// Location; failover follows the Send Port's policy*).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Attempts {
    /// The active Send Location, by its place in the Send Port's order.
    pub location: u32,
    /// How often the active Send Location has been tried.
    pub tries: u32,
}
