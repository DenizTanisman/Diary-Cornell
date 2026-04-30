//! Char-level CRDT engine (FAZ 3.1).
//!
//! A linked-list / RGA-flavoured CRDT: each character is a `CharNode` with
//! a stable `char_id`, a Lamport clock, and `prev_id`/`next_id` pointers.
//! Inserts and deletes are commutative when applied in any order; the only
//! tie-breaker is `peer_id` lexicographic order when two inserts target the
//! same `prev_id`.
//!
//! The same op stream applied to a Cloud (Python) document and a Diary
//! (Rust) document must materialize to identical text. The parity test in
//! `document.rs` exercises this with random op sequences, and FAZ 3.2 adds
//! a cross-implementation HTTP test against the Cloud server.

// FAZ 3.1 ships the engine + tests only. Tauri commands / WS pipeline
// land in FAZ 3.2 and will consume these types — the dead-code/unused-
// imports allow keeps clippy quiet during the in-between commit.
#![allow(dead_code, unused_imports)]

pub mod document;
pub mod node;
pub mod operations;

pub use document::CrdtDocument;
pub use node::CharNode;
pub use operations::CharOp;
