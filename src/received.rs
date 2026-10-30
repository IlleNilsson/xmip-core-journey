//! Where the Stream of a Journey's Message came in, and the sequences it holds a
//! place in end to end (ADR-0024 and ADR-0066, amendments 2026-10-22).
//!
//! The owner, 2026-10-22: *A default correlation set is its receive
//! location name. There might be more streams, files, pushing at that
//! receive location. Default one stream, message, journey at a time.* So a
//! Stream holds a place in its Receive Location's sequence from its
//! Publication until the last Journey born from it ends, and every Journey
//! born from it — the ones its Publication opened and the ones a Work
//! Process published from them — carries where it came in and which
//! sequences its Stream holds a place in, so every step can ask whether
//! that Stream heads them.

/// Where the Stream of a Journey's Message came in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Received {
    /// The Receive Location and its Receive Port.
    pub location: String,
    pub port: String,
    /// The Message the Stream was received as: what holds the place in
    /// each sequence.
    pub origin: u128,
    /// The sequences the Stream holds a place in, end to end, by their
    /// queues in the Ledger.
    pub sequences: Vec<u128>,
    /// Whether a Message of it that failed lets its places go, so the rest
    /// pass (`on_failure = "skip"`), where it keeps them until an operator
    /// retries or dismisses it (`block`, the default).
    pub skips: bool,
}

/// A place a Message holds in a sequence of the point it is at — its Send
/// Port's, a Send Location's it may leave by, or its Work Process's, each
/// naming a Correlation — taken once its Stream heads its own
/// sequences, and let go once it has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequenced {
    /// The sequence's queue in the Ledger.
    pub queue: u128,
    /// Whether a failure of it lets its place go (`on_failure = "skip"`).
    pub skips: bool,
}

/// A Journey's part in a delivery by a Send Port Group that names a
/// Correlation: one Journey per member Send Port, all of one delivery
/// holding one place in the Group's sequence for their key, until every
/// member has succeeded or failed (the owner, 2026-10-27: *The result is a
/// set of ports that succeeded or failed … one has to accumulate the
/// results*). Xmip Storage counts the members still open and lets the
/// place go with the last (`persist::storage`'s origin).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupDelivery {
    /// The Send Port Group.
    pub name: String,
    /// The delivery: what holds the place, one for the members a
    /// Publication opened for the Group.
    pub delivery: u128,
    /// The Group's sequence for this member's key, once its place is
    /// taken.
    pub queue: Option<u128>,
    /// Whether a member that failed lets the place go (`on_failure =
    /// "skip"`), where it keeps it until an operator retries or dismisses
    /// it (`block`, the default).
    pub skips: bool,
    /// Every member Journey of the delivery, in member order: what the set
    /// is reported over.
    pub members: Vec<u128>,
}
