//! A Journey's one binary form: what the Ledger keeps of it, as the body of
//! Xmip Storage's Journey record (`runtime-model.md` section 3, *The
//! Ledger*), and the Journey read back from it.
//!
//! **Binary, not JSON.** The Ledger is a database, written once per step of
//! every Journey, and the form is read by the next step's thread: a JSON
//! text would spend its bytes on field names and its time on a text parser
//! for every hand-on, and the estate keeps no JSON at rest (ADR-0031 clause
//! 3). So the fields are written in their order — no names, no padding:
//! identifiers as 16 bytes big-endian, a time as 16, a count, a length or a
//! depth as a varint, text as its length and its UTF-8 bytes, an absent
//! value as one byte saying so. The first byte is the form's number,
//! [`FORM`], so a form that follows can still read what this one wrote.
//!
//! One form, here, with the type it writes: Xmip Storage keeps the body as
//! it is given, byte for byte, and nothing else writes a Journey's.

use codec::CodecError;
use codec::cursor::Cursor;
use codec::field::{many, optional, place, placed, read_many, read_optional, read_text, text};
use codec::writer::ByteWriter;
use xcore::{ExecutionId, JourneyId, MessageId, StreamId};

use crate::{Attempts, ChainCause, Journey, JourneyEntry, JourneyMessageRef, JourneyState};

/// The form's number, the first byte of every record written in it.
pub const FORM: u8 = 2;

impl Journey {
    /// The Journey in its one binary form.
    #[must_use]
    pub fn record(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + 64 * self.entries.len());
        out.byte(FORM).u128_be(self.journey_id.value());
        out.byte(place(&STATES, &self.state));
        optional(&mut out, self.previous_journey_id, |out, id| {
            out.u128_be(id.value());
        });
        optional(&mut out, self.cause.as_ref(), |out, cause| {
            text(out, &cause.subscription_id);
            optional(out, cause.work_process.as_deref(), text);
        });
        out.varint(u64::from(self.depth));
        optional(&mut out, self.current_work_process.as_deref(), text);
        optional(&mut out, self.send_port.as_deref(), text);
        out.varint(u64::from(self.attempts.location))
            .varint(u64::from(self.attempts.tries));
        many(&mut out, &self.entries, |out, entry| {
            out.u128_be(entry.execution_id.value())
                .u128_be(entry.message_id.value());
            text(out, &entry.action);
            text(out, &entry.outcome);
            out.i128_be(entry.timestamp_unix_nanos);
        });
        many(&mut out, &self.messages, |out, held| {
            out.u128_be(held.message_id.value())
                .u128_be(held.stream_id.value());
        });
        out
    }

    /// The Journey `bytes` hold in its one binary form, and nothing after it.
    ///
    /// # Errors
    ///
    /// Where the bytes are not one: another form, a field cut short, a
    /// state with no number, text that is not UTF-8, or bytes after it.
    pub fn from_record(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = Cursor::new(bytes);
        let form = cursor.byte()?;
        if form != FORM {
            return Err(CodecError::new(format!(
                "a Journey record of form {form}, and this reads form {FORM}"
            )));
        }
        let journey_id = JourneyId::new(cursor.u128_be()?);
        let state = placed(&STATES, cursor.byte()?, "Journey state")?;
        let previous_journey_id = read_optional(&mut cursor, |c| Ok(JourneyId::new(c.u128_be()?)))?;
        let cause = read_optional(&mut cursor, |c| {
            Ok(ChainCause {
                subscription_id: read_text(c)?,
                work_process: read_optional(c, read_text)?,
            })
        })?;
        let depth = u32::try_from(cursor.varint()?)
            .map_err(|_| CodecError::new("a Journey's depth past u32"))?;
        let current_work_process = read_optional(&mut cursor, read_text)?;
        let send_port = read_optional(&mut cursor, read_text)?;
        let counted = |cursor: &mut Cursor<'_>, what: &str| {
            u32::try_from(cursor.varint()?)
                .map_err(|_| CodecError::new(format!("a Journey's {what} past u32")))
        };
        let attempts = Attempts {
            location: counted(&mut cursor, "Send Location")?,
            tries: counted(&mut cursor, "tries")?,
        };
        let entries = read_many(&mut cursor, |c| {
            Ok(JourneyEntry {
                execution_id: ExecutionId::new(c.u128_be()?),
                message_id: MessageId::new(c.u128_be()?),
                action: read_text(c)?,
                outcome: read_text(c)?,
                timestamp_unix_nanos: c.i128_be()?,
            })
        })?;
        let messages = read_many(&mut cursor, |c| {
            Ok(JourneyMessageRef {
                message_id: MessageId::new(c.u128_be()?),
                stream_id: StreamId::new(c.u128_be()?),
            })
        })?;
        if !cursor.is_empty() {
            return Err(CodecError::new("bytes after the Journey record"));
        }
        Ok(Self {
            journey_id,
            state,
            previous_journey_id,
            cause,
            depth,
            current_work_process,
            send_port,
            attempts,
            entries,
            messages,
        })
    }
}

/// Every state, in the order the form numbers them, which only grows at its
/// end.
const STATES: [JourneyState; 7] = [
    JourneyState::Active,
    JourneyState::Waiting,
    JourneyState::Suspended,
    JourneyState::Recovering,
    JourneyState::Completed,
    JourneyState::Failed,
    JourneyState::Dismissed,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChainLimit;

    fn travelled() -> Journey {
        let first = Journey::new(JourneyId::new(1));
        let mut journey = Journey::following(
            JourneyId::new(0x0199_0000_0000_7000_8000_0000_0000_0002),
            &first,
            ChainCause::process("billing", "Approval"),
            ChainLimit::DEFAULT,
        )
        .expect("within the limit")
        .holding(JourneyMessageRef {
            message_id: MessageId::new(7),
            stream_id: StreamId::new(9),
        })
        .append(
            JourneyEntry {
                execution_id: ExecutionId::new(3),
                message_id: MessageId::new(7),
                action: "send".to_string(),
                outcome: "sent to SendPort.Billing — åäö".to_string(),
                timestamp_unix_nanos: -12,
            },
            JourneyState::Waiting,
        );
        journey.current_work_process = Some("Approval".to_string());
        journey.send_port = Some("Billing".to_string());
        journey.attempts = Attempts {
            location: 1,
            tries: 300,
        };
        journey
    }

    #[test]
    fn a_journey_keeps_its_send_port_and_its_retry_history() {
        let read = Journey::from_record(&travelled().record()).expect("read");
        assert_eq!(read.send_port.as_deref(), Some("Billing"));
        assert_eq!(
            read.attempts,
            Attempts {
                location: 1,
                tries: 300
            },
            "a retry's count survives the Ledger"
        );
    }

    #[test]
    fn a_journey_comes_back_from_its_record_as_it_was() {
        for journey in [Journey::new(JourneyId::new(5)), travelled()] {
            let record = journey.record();
            assert_eq!(record[0], FORM);
            assert_eq!(Journey::from_record(&record).expect("read"), journey);
        }
        for state in STATES {
            let mut journey = Journey::new(JourneyId::new(5));
            journey.state = state;
            let read = Journey::from_record(&journey.record()).expect("read");
            assert_eq!(read.state, state);
        }
    }

    #[test]
    fn a_journey_read_back_keeps_its_chain_and_the_limit_still_holds() {
        // Problem 25, row a: a copy without the depth came back at zero
        // and the limit restarted.
        let limit = ChainLimit::new(2);
        let cause = || ChainCause::subscription("orders");
        let first = Journey::new(JourneyId::new(20));
        let second =
            Journey::following(JourneyId::new(21), &first, cause(), limit).expect("one link");
        let third =
            Journey::following(JourneyId::new(22), &second, cause(), limit).expect("two links");
        let read = Journey::from_record(&third.record()).expect("read");
        assert_eq!(read.depth(), 2);
        assert_eq!(read.previous_journey_id(), Some(JourneyId::new(21)));
        let refused = Journey::following(JourneyId::new(23), &read, cause(), limit);
        assert!(refused.is_err(), "the limit counts the links made before");
    }

    #[test]
    fn the_record_is_compact() {
        // A Journey as receive opens it: its identifier, its Subscription
        // and the Message it holds, in under a hundred bytes.
        let opened = Journey::matched(JourneyId::new(5), ChainCause::subscription("billing"))
            .holding(JourneyMessageRef {
                message_id: MessageId::new(7),
                stream_id: StreamId::new(9),
            });
        assert!(opened.record().len() < 100, "{}", opened.record().len());
    }

    #[test]
    fn bytes_that_are_not_a_record_are_refused() {
        let record = travelled().record();
        for cut in [0, 1, 17, record.len() - 1] {
            assert!(Journey::from_record(&record[..cut]).is_err(), "{cut}");
        }
        let longer = [record.as_slice(), &[0]].concat();
        assert!(Journey::from_record(&longer).is_err());
        let mut other_form = record.clone();
        other_form[0] = FORM + 1;
        let refused = Journey::from_record(&other_form).expect_err("another form");
        assert!(refused.message.contains("form 3"), "{refused}");
        let mut no_state = record;
        no_state[17] = 9;
        assert!(Journey::from_record(&no_state).is_err());
    }
}
