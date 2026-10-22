//! Where a Journey's Stream came in, and the sequences it holds a place in
//! end to end (ADR-0024 and ADR-0066, amendments 2026-10-22).
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

/// Where a Journey's Stream came in.
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
    /// Whether a Journey of it that failed lets its places go, so the rest
    /// pass (`on_failure = "skip"`), where it keeps them until an operator
    /// retries or dismisses it (`block`, the default).
    pub skips: bool,
}

/// The place a Journey holds in the sequence of the point it is at — a
/// Send Port's or a Work Process's that names a correlation set — taken in
/// the write that brought it there, and let go once it has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequenced {
    /// The sequence's queue in the Ledger.
    pub queue: u128,
    /// Whether a failure of it lets its place go (`on_failure = "skip"`).
    pub skips: bool,
}
