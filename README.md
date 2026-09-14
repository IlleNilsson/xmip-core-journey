# xmip-core-journey

The Journey: one line of execution from a matched Subscription to a terminal
state, and the record of what happened on it — `JourneyState`, the entries
appended as it runs, the Messages it references and the link to the Journey
before it.

A Journey is a line, not a tree: a Publication produces one Journey per
matched Subscription, and a Journey exists only after Validation. It references
Messages and never the reverse, because one Message may belong to several
Journeys. It is not the Message, not the Stream and not the Xmip Process that
runs inside it.

ADR-0013 governs the Journey and its states; `doc/architecture/runtime-model.md`
sections 12 to 14 are state, control and replay. `architecture.toml` carries
the maturity.
