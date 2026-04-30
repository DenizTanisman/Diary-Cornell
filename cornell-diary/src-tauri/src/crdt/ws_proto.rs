//! Wire-level message envelopes for the Cloud WS endpoint
//! (`ws://cloud/ws/journal/{journal_id}?token=JWT`).
//!
//! The encoding is `{"type":"<variant>", ...payload}`. We mirror Cloud's
//! contract from the FAZ 3 prompt §"WebSocket (FAZ 3)" exactly so the
//! same JSON works in both directions.

use serde::{Deserialize, Serialize};

use crate::crdt::operations::CharOp;

/// Messages Diary sends to Cloud.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsOut {
    /// Asks the server to start mirroring CRDT ops for one entry on this
    /// open socket. After this lands the server's `presence_update`
    /// includes our peer.
    Subscribe { entry_date: String },
    /// Broadcasts one local op for one (entry, field) pair.
    CrdtOp {
        entry_date: String,
        field: String,
        op: CharOp,
    },
    /// Heartbeat / presence ping. Server may reply with the latest
    /// `presence_update`; sent on a timer so the connection self-closes
    /// if a network blip happens.
    #[allow(dead_code)] // wired in FAZ 3.3 where the keepalive timer lives
    Presence,
}

/// Messages Cloud sends to Diary.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsIn {
    /// Another peer's op, fanned out to every subscriber of the same
    /// entry. Idempotent — replaying the same op is a no-op in
    /// `CrdtDocument::apply_remote`.
    CrdtOpBroadcast {
        entry_date: String,
        field: String,
        op: CharOp,
        #[serde(default)]
        from_peer: Option<String>,
    },
    /// Set of peer ids currently subscribed to the same entry. We use
    /// `len() > 1` to flip the React UI into "live edit" mode.
    PresenceUpdate { peers: Vec<String> },
    /// Cloud just materialised a new snapshot of this entry to disk.
    /// Diary uses this to invalidate any in-memory cache and trigger a
    /// pull on the next idle moment.
    SnapshotUpdated { entry_date: String, version: i64 },
    /// Server-side error. Logged but not surfaced unless it kills the
    /// connection.
    Error {
        #[serde(default)]
        code: Option<String>,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::node::CharNode;

    #[test]
    fn ws_out_subscribe_serialises_with_type_tag() {
        let m = WsOut::Subscribe {
            entry_date: "2026-04-29".into(),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["type"], "subscribe");
        assert_eq!(v["entry_date"], "2026-04-29");
    }

    #[test]
    fn ws_out_crdt_op_includes_field_and_op() {
        let node = CharNode::new("alice", 1, 0, 'x', None);
        let op = CharOp::from_insert(&node);
        let m = WsOut::CrdtOp {
            entry_date: "2026-04-29".into(),
            field: "diary".into(),
            op,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["type"], "crdt_op");
        assert_eq!(v["field"], "diary");
        assert_eq!(v["op"]["op_type"], "insert");
    }

    #[test]
    fn ws_in_decodes_crdt_op_broadcast() {
        let json = r#"{
            "type": "crdt_op_broadcast",
            "entry_date": "2026-04-29",
            "field": "diary",
            "op": {
                "op_type": "insert",
                "char_id": "bob-1-0",
                "char_value": "y",
                "peer_id": "bob",
                "lamport": 1,
                "seq": 0,
                "prev_id": null
            },
            "from_peer": "bob"
        }"#;
        let m: WsIn = serde_json::from_str(json).unwrap();
        match m {
            WsIn::CrdtOpBroadcast {
                entry_date,
                field,
                op,
                from_peer,
            } => {
                assert_eq!(entry_date, "2026-04-29");
                assert_eq!(field, "diary");
                assert_eq!(from_peer.as_deref(), Some("bob"));
                match op {
                    CharOp::Insert { char_id, .. } => assert_eq!(char_id, "bob-1-0"),
                    _ => panic!("expected insert op"),
                }
            }
            _ => panic!("expected CrdtOpBroadcast"),
        }
    }

    #[test]
    fn ws_in_decodes_presence_update() {
        let json = r#"{"type":"presence_update","peers":["alice@laptop","bob@phone"]}"#;
        let m: WsIn = serde_json::from_str(json).unwrap();
        match m {
            WsIn::PresenceUpdate { peers } => assert_eq!(peers, vec!["alice@laptop", "bob@phone"]),
            _ => panic!("expected PresenceUpdate"),
        }
    }

    #[test]
    fn ws_in_decodes_snapshot_updated() {
        let json = r#"{"type":"snapshot_updated","entry_date":"2026-04-29","version":42}"#;
        let m: WsIn = serde_json::from_str(json).unwrap();
        match m {
            WsIn::SnapshotUpdated { version, .. } => assert_eq!(version, 42),
            _ => panic!("expected SnapshotUpdated"),
        }
    }

    #[test]
    fn ws_in_decodes_error() {
        let json = r#"{"type":"error","code":"unauthorized","message":"bad token"}"#;
        let m: WsIn = serde_json::from_str(json).unwrap();
        match m {
            WsIn::Error { code, message } => {
                assert_eq!(code.as_deref(), Some("unauthorized"));
                assert_eq!(message, "bad token");
            }
            _ => panic!("expected Error"),
        }
    }
}
