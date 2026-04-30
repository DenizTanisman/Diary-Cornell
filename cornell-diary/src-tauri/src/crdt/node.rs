//! `CharNode` — one character plus its placement metadata.
//!
//! `char_id` is `"{peer_id}-{lamport}-{seq}"`. peer_id alone isn't unique
//! across two inserts in the same Lamport tick (a peer might insert two
//! chars with the same lamport before another tick lands), so `seq` breaks
//! that tie locally. The full triple is therefore globally unique without
//! coordination.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharNode {
    pub char_id: String,
    /// Single Unicode scalar value. Cloud's CRDTOpDTO.char_value caps at
    /// 8 bytes (so multi-byte codepoints fit) — we store one `char` here
    /// and serialise as a string when crossing the wire.
    pub character: char,
    pub peer_id: String,
    pub lamport: u64,
    pub seq: u32,
    /// `None` for the document head; `Some(head_id)` for the first real
    /// char. Always `Some` for tail-anchored inserts.
    pub prev_id: Option<String>,
    /// Cached next pointer. Recomputed during apply; not authoritative on
    /// the wire — receivers can derive it from prev_id chains.
    pub next_id: Option<String>,
    pub is_deleted: bool,
}

impl CharNode {
    /// Convenience constructor for tests and local inserts.
    pub fn new(
        peer_id: &str,
        lamport: u64,
        seq: u32,
        character: char,
        prev_id: Option<String>,
    ) -> Self {
        Self {
            char_id: format!("{peer_id}-{lamport}-{seq}"),
            character,
            peer_id: peer_id.to_string(),
            lamport,
            seq,
            prev_id,
            next_id: None,
            is_deleted: false,
        }
    }
}
