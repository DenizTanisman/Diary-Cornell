//! `CharOp` — the wire envelope for a single CRDT mutation.
//!
//! Tagged enum on the Rust side, but **on the wire** we serialise into
//! Cloud's flat `CRDTOpDTO` shape so the same JSON works in both directions
//! without a translation layer:
//!
//! ```text
//! { "op_type": "insert", "char_id": "...", "char_value": "x",
//!   "prev_id": "...", "peer_id": "...", "lamport": 42, "seq": 0,
//!   "entry_id": "<uuid>", "field_name": "diary" }
//! { "op_type": "delete", "char_id": "...", "peer_id": "...",
//!   "lamport": 43, "entry_id": "...", "field_name": "..." }
//! ```
//!
//! `entry_id` and `field_name` belong to the per-op envelope (which entry
//! / which column the char lives in) — they're carried alongside but not
//! used by the engine itself, which operates on a single document at a
//! time.

use serde::{Deserialize, Serialize};

use crate::crdt::node::CharNode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op_type", rename_all = "snake_case")]
pub enum CharOp {
    Insert {
        char_id: String,
        /// One char serialised as a one-character string.
        #[serde(rename = "char_value")]
        character: CharString,
        peer_id: String,
        lamport: u64,
        #[serde(default)]
        seq: u32,
        #[serde(default)]
        prev_id: Option<String>,
    },
    Delete {
        char_id: String,
        peer_id: String,
        lamport: u64,
    },
}

impl CharOp {
    pub fn from_insert(node: &CharNode) -> Self {
        // Internally we normalise head-anchored nodes to prev_id =
        // Some(HEAD_ID). On the wire, head-anchored ops use None so
        // Cloud's CRDTOpDTO matches.
        let prev_id = match node.prev_id.as_deref() {
            Some(crate::crdt::document::HEAD_ID) => None,
            other => other.map(str::to_string),
        };
        CharOp::Insert {
            char_id: node.char_id.clone(),
            character: CharString(node.character),
            peer_id: node.peer_id.clone(),
            lamport: node.lamport,
            seq: node.seq,
            prev_id,
        }
    }

    pub fn delete(char_id: String, peer_id: String, lamport: u64) -> Self {
        CharOp::Delete {
            char_id,
            peer_id,
            lamport,
        }
    }

    pub fn lamport(&self) -> u64 {
        match self {
            CharOp::Insert { lamport, .. } | CharOp::Delete { lamport, .. } => *lamport,
        }
    }

    pub fn char_id(&self) -> &str {
        match self {
            CharOp::Insert { char_id, .. } | CharOp::Delete { char_id, .. } => char_id,
        }
    }
}

/// Serde wrapper that encodes a single `char` as a one-character string.
/// Cloud's wire format uses strings for char_value (up to 8 bytes) so a
/// multi-byte UTF-8 codepoint fits without surprise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharString(pub char);

impl Serialize for CharString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; 4];
        s.serialize_str(self.0.encode_utf8(&mut buf))
    }
}

impl<'de> Deserialize<'de> for CharString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let mut iter = s.chars();
        match (iter.next(), iter.next()) {
            (Some(c), None) => Ok(CharString(c)),
            _ => Err(serde::de::Error::custom(format!(
                "char_value must be exactly one character, got {s:?}"
            ))),
        }
    }
}
