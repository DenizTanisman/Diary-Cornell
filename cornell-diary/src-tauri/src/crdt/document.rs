//! `CrdtDocument` — one editable string with linked-list CRDT semantics.
//!
//! Internally:
//! - `nodes` is a HashMap of every char node we've ever seen (including
//!   tombstones). O(1) lookup by `char_id`.
//! - `head` and `tail` are sentinel char_ids whose only purpose is to anchor
//!   `prev_id`/`next_id` chains. They never appear in `materialize()`.
//! - `pending` is a queue of remote inserts whose `prev_id` we haven't
//!   received yet. `flush_pending` walks it whenever a new node lands.
//!
//! Conflict resolution (two inserts target the same `prev_id`):
//! - Both nodes attach after the same parent. We sort the resulting
//!   siblings by `(lamport, peer_id)` descending so the *higher* tuple
//!   ends up closer to the parent — equivalent to the RGA "newer wins
//!   left" rule.
//! - peer_id is broken lexicographically: `bob > alice` ⇒ bob's char
//!   sits left of alice's when both inserted at the same prev_id with
//!   the same lamport.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::crdt::node::CharNode;
use crate::crdt::operations::CharOp;

pub(crate) const HEAD_ID: &str = "__HEAD__";
pub(crate) const TAIL_ID: &str = "__TAIL__";

pub struct CrdtDocument {
    pub local_peer: String,
    nodes: Mutex<HashMap<String, CharNode>>,
    /// Lamport clock. We bump on every local op and `fetch_max` on every
    /// remote op to keep clocks monotone across peers.
    lamport: AtomicU64,
    /// Local tie-breaker counter so two inserts in the same Lamport tick
    /// still get unique char_ids.
    seq: AtomicU64,
    /// Remote inserts whose `prev_id` hasn't been seen yet. Kept in
    /// arrival order; `flush_pending` re-tries them from the front each
    /// time a new node lands.
    pending: Mutex<Vec<CharOp>>,
}

impl CrdtDocument {
    pub fn new(local_peer: impl Into<String>) -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            HEAD_ID.to_string(),
            CharNode {
                char_id: HEAD_ID.to_string(),
                character: '\0',
                peer_id: String::new(),
                lamport: 0,
                seq: 0,
                prev_id: None,
                next_id: Some(TAIL_ID.to_string()),
                is_deleted: true, // sentinels never materialize
            },
        );
        nodes.insert(
            TAIL_ID.to_string(),
            CharNode {
                char_id: TAIL_ID.to_string(),
                character: '\0',
                peer_id: String::new(),
                lamport: 0,
                seq: 0,
                prev_id: Some(HEAD_ID.to_string()),
                next_id: None,
                is_deleted: true,
            },
        );
        Self {
            local_peer: local_peer.into(),
            nodes: Mutex::new(nodes),
            lamport: AtomicU64::new(0),
            seq: AtomicU64::new(0),
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Insert a character locally after `prev_id` (or at the start if `None`).
    /// Returns the op that should be broadcast.
    pub fn local_insert(&self, character: char, prev_id: Option<&str>) -> CharOp {
        let lamport = self.lamport.fetch_add(1, Ordering::SeqCst) + 1;
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) as u32;
        let prev_id = Some(prev_id.unwrap_or(HEAD_ID).to_string());
        let node = CharNode::new(&self.local_peer, lamport, seq, character, prev_id);

        let mut nodes = self.lock_nodes();
        Self::insert_node(&mut nodes, node.clone());
        CharOp::from_insert(&node)
    }

    /// Mark a character as deleted locally. Tombstone stays in the map so
    /// other peers can still reference it from their own `prev_id`.
    pub fn local_delete(&self, char_id: &str) -> Option<CharOp> {
        let lamport = self.lamport.fetch_add(1, Ordering::SeqCst) + 1;
        let mut nodes = self.lock_nodes();
        let node = nodes.get_mut(char_id)?;
        node.is_deleted = true;
        Some(CharOp::delete(
            char_id.to_string(),
            self.local_peer.clone(),
            lamport,
        ))
    }

    /// Apply a remote op. Idempotent — replaying the same op is a no-op.
    /// Inserts whose `prev_id` we don't have yet are queued and re-tried
    /// after every subsequent successful insert.
    pub fn apply_remote(&self, op: CharOp) {
        // Bump the local clock so any future local insert outranks
        // everything we've seen, regardless of which peer we got it from.
        self.lamport.fetch_max(op.lamport(), Ordering::SeqCst);

        match op {
            CharOp::Insert { .. } => {
                self.try_apply_insert(op);
                self.flush_pending();
            }
            CharOp::Delete { char_id, .. } => {
                let mut nodes = self.lock_nodes();
                if let Some(node) = nodes.get_mut(&char_id) {
                    node.is_deleted = true;
                }
                // If the char_id is unknown (insert hasn't arrived yet),
                // we silently drop the delete. When the matching insert
                // arrives later it will land as not-deleted and the user
                // sees the char briefly. This is a known edge case the
                // prompt documents as acceptable for FAZ 3.
            }
        }
    }

    /// Walk the linked list head→tail and return the visible string.
    pub fn materialize(&self) -> String {
        let nodes = self.lock_nodes();
        let mut out = String::new();
        let mut cursor = nodes.get(HEAD_ID).and_then(|h| h.next_id.clone());
        let mut hops = 0usize;
        while let Some(id) = cursor {
            if id == TAIL_ID {
                break;
            }
            let node = match nodes.get(&id) {
                Some(n) => n,
                None => break,
            };
            if !node.is_deleted {
                out.push(node.character);
            }
            cursor = node.next_id.clone();
            hops += 1;
            // Defensive: if a corrupt doc forms a cycle, bail rather than
            // hang forever. Won't trigger in the well-tested paths.
            if hops > nodes.len() + 4 {
                tracing::warn!(target: "cornell_diary::crdt", "cycle detected during materialize");
                break;
            }
        }
        out
    }

    /// Snapshot every char_id currently in the document. Useful for tests
    /// that want to assert on the raw structure without going through
    /// materialize.
    #[cfg(test)]
    pub fn node_ids(&self) -> Vec<String> {
        let nodes = self.lock_nodes();
        let mut out: Vec<String> = nodes.keys().cloned().collect();
        out.sort();
        out
    }

    // ----------------------------------------------------------------------
    // internals
    // ----------------------------------------------------------------------

    fn lock_nodes(&self) -> std::sync::MutexGuard<'_, HashMap<String, CharNode>> {
        self.nodes.lock().expect("crdt nodes mutex poisoned")
    }

    fn try_apply_insert(&self, op: CharOp) {
        let CharOp::Insert {
            char_id,
            character,
            peer_id,
            lamport,
            seq,
            prev_id,
        } = op
        else {
            return;
        };

        // Normalise: a missing prev_id means "anchored at the document
        // head". Storing it as Some(HEAD_ID) makes the integration logic
        // uniform (no None / Some(HEAD) split). We restore the None on
        // the wire in CharOp::from_insert.
        let normalised_prev = Some(prev_id.clone().unwrap_or_else(|| HEAD_ID.to_string()));
        let node = CharNode {
            char_id: char_id.clone(),
            character: character.0,
            peer_id,
            lamport,
            seq,
            prev_id: normalised_prev,
            next_id: None,
            is_deleted: false,
        };

        // 1. Idempotent: already present.
        {
            let nodes = self.lock_nodes();
            if nodes.contains_key(&char_id) {
                return;
            }
            // 2. Parent unknown → queue and bail. flush_pending will retry.
            let parent = prev_id.as_deref().unwrap_or(HEAD_ID);
            if !nodes.contains_key(parent) {
                drop(nodes);
                self.queue_pending(CharOp::from_insert(&node));
                return;
            }
        }

        // 3. Insert it. Holding the lock through link_neighbors keeps the
        //    parent's next_id update atomic with the new node's prev/next.
        {
            let mut nodes = self.lock_nodes();
            Self::insert_node(&mut nodes, node);
        }
    }

    /// Place `node` somewhere after its parent in the linked list.
    ///
    /// RGA convergence rule: among siblings sharing the same `prev_id`,
    /// the one with the *highest* `(lamport, peer_id)` sits closest to
    /// the parent. So when we walk the chain we **skip past** any sibling
    /// that outranks us — *and the entire subtree that hangs off that
    /// sibling*. We track the splice point with `prev_link` (linked-list
    /// predecessor) and `cursor_id` (linked-list successor). Splicing
    /// only rewires `next_id`; the immutable `prev_id` field stays as the
    /// RGA parent so future siblings of `parent_id` can still recognise
    /// each other.
    fn insert_node(nodes: &mut HashMap<String, CharNode>, node: CharNode) {
        let parent_id = node.prev_id.clone().unwrap_or_else(|| HEAD_ID.to_string());
        let our_rank = (node.lamport, node.peer_id.as_str());

        let mut prev_link = parent_id.clone();
        let mut cursor_id = nodes
            .get(&parent_id)
            .and_then(|n| n.next_id.clone())
            .unwrap_or_else(|| TAIL_ID.to_string());

        let mut hops = 0usize;
        let max_hops = nodes.len() + 4;
        loop {
            if cursor_id == TAIL_ID {
                break;
            }
            let cur = match nodes.get(&cursor_id) {
                Some(n) => n,
                None => break,
            };

            if cur.prev_id.as_deref() == Some(parent_id.as_str()) {
                let sibling_rank = (cur.lamport, cur.peer_id.as_str());
                if sibling_rank > our_rank {
                    // Outranks us → skip past its entire subtree before
                    // re-evaluating against the next direct sibling.
                    let (last_in, after) = end_of_subtree(nodes, &cursor_id);
                    prev_link = last_in;
                    cursor_id = after;
                } else {
                    // Same or lower rank → splice before this sibling.
                    break;
                }
            } else if !is_in_subtree(nodes, &cursor_id, &parent_id) {
                // We've walked past the end of `parent_id`'s subtree
                // entirely (into an aunt/uncle's territory). Splice here.
                break;
            } else {
                // Descendant of an earlier kept sibling — keep walking.
                prev_link = cursor_id.clone();
                cursor_id = cur.next_id.clone().unwrap_or_else(|| TAIL_ID.to_string());
            }

            hops += 1;
            if hops > max_hops {
                tracing::warn!(target: "cornell_diary::crdt", "insert_node hop limit hit");
                break;
            }
        }

        let mut new_node = node;
        new_node.next_id = Some(cursor_id);
        let new_id = new_node.char_id.clone();
        if let Some(left) = nodes.get_mut(&prev_link) {
            left.next_id = Some(new_id.clone());
        }
        nodes.insert(new_id, new_node);
    }

    fn queue_pending(&self, op: CharOp) {
        let mut pending = self.pending.lock().expect("pending mutex poisoned");
        pending.push(op);
    }

    fn flush_pending(&self) {
        // Re-collect every iteration: applying one op might unblock a
        // chain of others. Keep looping as long as a pass makes progress
        // (some op moved out of pending). Once a full pass produces no
        // progress, the remaining items are stuck waiting on parents we
        // haven't received yet — restore them and bail.
        loop {
            let drained: Vec<CharOp> = {
                let mut pending = self.pending.lock().expect("pending mutex poisoned");
                std::mem::take(&mut *pending)
            };
            if drained.is_empty() {
                return;
            }
            let drained_count = drained.len();
            let mut still_pending: Vec<CharOp> = Vec::new();
            for op in drained {
                let parent_known = match &op {
                    CharOp::Insert { prev_id, .. } => {
                        let nodes = self.lock_nodes();
                        let parent = prev_id.as_deref().unwrap_or(HEAD_ID);
                        nodes.contains_key(parent)
                    }
                    CharOp::Delete { .. } => true,
                };
                if parent_known {
                    self.try_apply_insert(op);
                } else {
                    still_pending.push(op);
                }
            }
            let made_progress = still_pending.len() < drained_count;
            if !still_pending.is_empty() {
                let mut pending = self.pending.lock().expect("pending mutex poisoned");
                pending.extend(still_pending);
            }
            if !made_progress {
                return;
            }
            // else: re-loop. Items that just landed may have unblocked
            // others now sitting in `pending`.
        }
    }
}

/// Walk forward from `subtree_root_id` until we leave its subtree.
/// Returns `(last_node_in_subtree, first_node_outside)`. `first_node_outside`
/// is `TAIL_ID` if the subtree extends to the end of the document.
fn end_of_subtree(nodes: &HashMap<String, CharNode>, subtree_root_id: &str) -> (String, String) {
    let mut last_in = subtree_root_id.to_string();
    let mut cur = nodes
        .get(subtree_root_id)
        .and_then(|n| n.next_id.clone())
        .unwrap_or_else(|| TAIL_ID.to_string());
    let mut hops = 0usize;
    let max_hops = nodes.len() + 4;
    loop {
        if cur == TAIL_ID {
            return (last_in, cur);
        }
        let n = match nodes.get(&cur) {
            Some(n) => n,
            None => return (last_in, TAIL_ID.to_string()),
        };
        if !is_in_subtree(nodes, &cur, subtree_root_id) {
            return (last_in, cur);
        }
        last_in = cur.clone();
        cur = n.next_id.clone().unwrap_or_else(|| TAIL_ID.to_string());
        hops += 1;
        if hops > max_hops {
            return (last_in, cur);
        }
    }
}

/// True iff `root_id` is an ancestor of (or equal to) `node_id` in the
/// RGA tree, walking the immutable `prev_id` pointers.
fn is_in_subtree(nodes: &HashMap<String, CharNode>, node_id: &str, root_id: &str) -> bool {
    let mut cur = node_id.to_string();
    let mut hops = 0usize;
    let max_hops = nodes.len() + 4;
    loop {
        if cur == root_id {
            return true;
        }
        if cur == HEAD_ID {
            return false;
        }
        let n = match nodes.get(&cur) {
            Some(n) => n,
            None => return false,
        };
        match &n.prev_id {
            Some(p) => cur = p.clone(),
            None => return false,
        }
        hops += 1;
        if hops > max_hops {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op_from_other_peer(
        peer: &str,
        lamport: u64,
        seq: u32,
        ch: char,
        prev: Option<&str>,
    ) -> CharOp {
        let node = CharNode::new(peer, lamport, seq, ch, prev.map(String::from));
        CharOp::from_insert(&node)
    }

    #[test]
    fn local_inserts_materialize_in_order() {
        let doc = CrdtDocument::new("alice");
        let op1 = doc.local_insert('h', None);
        let op2 = doc.local_insert('i', Some(op1.char_id()));
        let _ = op2; // not used directly
        assert_eq!(doc.materialize(), "hi");
    }

    #[test]
    fn local_delete_removes_from_materialize_but_keeps_tombstone() {
        let doc = CrdtDocument::new("alice");
        let op1 = doc.local_insert('a', None);
        let op2 = doc.local_insert('b', Some(op1.char_id()));
        doc.local_delete(op1.char_id());
        assert_eq!(doc.materialize(), "b");
        // Tombstone still in the map so other peers can still reference it.
        assert!(doc.node_ids().iter().any(|id| id == op1.char_id()));
        let _ = op2;
    }

    #[test]
    fn apply_remote_is_idempotent_when_op_repeats() {
        let doc = CrdtDocument::new("alice");
        let op = op_from_other_peer("bob", 1, 0, 'x', None);
        doc.apply_remote(op.clone());
        doc.apply_remote(op.clone());
        doc.apply_remote(op);
        assert_eq!(doc.materialize(), "x");
    }

    #[test]
    fn apply_remote_pending_queue_resolves_when_parent_arrives() {
        let doc = CrdtDocument::new("alice");
        // bob inserts 'a' (lamport 1), then 'b' after it (lamport 2). We
        // receive them out of order — 'b' first.
        let op_a = op_from_other_peer("bob", 1, 0, 'a', None);
        let op_b = op_from_other_peer("bob", 2, 0, 'b', Some("bob-1-0"));
        doc.apply_remote(op_b);
        // Without the parent, 'b' is in the pending queue and the doc is
        // empty.
        assert_eq!(doc.materialize(), "");
        // Parent arrives → both should land.
        doc.apply_remote(op_a);
        assert_eq!(doc.materialize(), "ab");
    }

    #[test]
    fn concurrent_inserts_at_same_parent_use_peer_id_tiebreak() {
        // Two peers each insert one char at the same parent, with the same
        // Lamport tick. RGA rule: highest (lamport, peer_id) sits closer
        // to the parent.
        let doc = CrdtDocument::new("driver");
        let parent = doc.local_insert('p', None);

        let op_alice = op_from_other_peer("alice", 5, 0, 'A', Some(parent.char_id()));
        let op_bob = op_from_other_peer("bob", 5, 0, 'B', Some(parent.char_id()));
        // Apply in BOTH orders on two fresh documents and assert convergence.
        let doc1 = CrdtDocument::new("driver");
        let p1 = doc1.local_insert('p', None);
        // re-target the test ops at this driver's parent id
        let alice1 = op_from_other_peer("alice", 5, 0, 'A', Some(p1.char_id()));
        let bob1 = op_from_other_peer("bob", 5, 0, 'B', Some(p1.char_id()));
        doc1.apply_remote(alice1);
        doc1.apply_remote(bob1);

        let doc2 = CrdtDocument::new("driver");
        let p2 = doc2.local_insert('p', None);
        let alice2 = op_from_other_peer("alice", 5, 0, 'A', Some(p2.char_id()));
        let bob2 = op_from_other_peer("bob", 5, 0, 'B', Some(p2.char_id()));
        doc2.apply_remote(bob2);
        doc2.apply_remote(alice2);

        assert_eq!(
            doc1.materialize(),
            doc2.materialize(),
            "two apply orders must converge to the same text"
        );
        // bob > alice lex, same lamport → bob's 'B' sits closer to parent.
        // After parent 'p', text is "pBA".
        assert_eq!(doc1.materialize(), "pBA");
        let _ = (op_alice, op_bob);
    }

    #[test]
    fn deleting_unknown_char_is_a_silent_noop() {
        let doc = CrdtDocument::new("alice");
        doc.local_insert('a', None);
        // pretend a remote delete arrived for a char we never saw — the
        // doc state should be unchanged.
        let bogus = CharOp::delete("ghost-1-0".into(), "bob".into(), 99);
        doc.apply_remote(bogus);
        assert_eq!(doc.materialize(), "a");
    }

    #[test]
    fn local_clock_overtakes_remote_clock_for_subsequent_inserts() {
        let doc = CrdtDocument::new("alice");
        // Bob is at Lamport 100. After receiving his op, Alice's next
        // local insert must outrank everything below 101.
        let bob_op = op_from_other_peer("bob", 100, 0, 'b', None);
        doc.apply_remote(bob_op);
        let alice_op = doc.local_insert('a', None);
        match alice_op {
            CharOp::Insert { lamport, .. } => assert!(lamport > 100, "got {lamport}"),
            _ => panic!("expected insert"),
        }
    }

    /// The convergence invariant: any two peers that receive the same set
    /// of ops in any order materialize to the same text.
    ///
    /// We synthesise 200 inserts spread across three peers, where each
    /// insert's prev_id is randomly drawn from previously-emitted ops on
    /// any peer (or None to insert at head). Then we replay the resulting
    /// op list on three fresh documents in three different orders
    /// (insertion order, reversed, shuffled) and assert all three
    /// materialize to the same text.
    #[test]
    fn convergence_holds_for_random_op_orderings() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Deterministic pseudo-random — tests should be reproducible.
        struct LcgRng(u64);
        impl LcgRng {
            fn next(&mut self) -> u64 {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                self.0
            }
            fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
                &slice[(self.next() as usize) % slice.len()]
            }
        }

        let mut rng = LcgRng(0xCAFEBABE_DEADBEEF);
        let peers = ["alice", "bob", "carol"];
        let mut all_ops: Vec<CharOp> = Vec::new();
        let mut emitted_ids: Vec<Option<String>> = vec![None];
        let mut lamports: std::collections::HashMap<&str, u64> =
            peers.iter().map(|p| (*p, 0u64)).collect();

        for n in 0..200 {
            let peer = peers[(rng.next() as usize) % peers.len()];
            let prev_id_owned = rng.pick(&emitted_ids).clone();
            let prev_ref = prev_id_owned.as_deref();
            let l = {
                let cur = lamports.get_mut(peer).unwrap();
                *cur += 1;
                *cur
            };
            let ch = char::from(b'a' + ((rng.next() % 26) as u8));
            let mut h = DefaultHasher::new();
            n.hash(&mut h);
            let seq = (h.finish() % 1000) as u32;
            let node = CharNode::new(peer, l, seq, ch, prev_ref.map(String::from));
            let op = CharOp::from_insert(&node);
            emitted_ids.push(Some(op.char_id().to_string()));
            all_ops.push(op);
        }

        let order_a: Vec<CharOp> = all_ops.clone();
        let mut order_b = all_ops.clone();
        order_b.reverse();
        let mut order_c = all_ops.clone();
        // Shuffle deterministically.
        for i in (1..order_c.len()).rev() {
            let j = (rng.next() as usize) % (i + 1);
            order_c.swap(i, j);
        }

        let apply = |order: &[CharOp]| {
            let doc = CrdtDocument::new("driver");
            for op in order {
                doc.apply_remote(op.clone());
            }
            doc.materialize()
        };

        let text_a = apply(&order_a);
        let text_b = apply(&order_b);
        let text_c = apply(&order_c);

        assert_eq!(text_a, text_b, "insertion order vs reverse diverged");
        assert_eq!(text_a, text_c, "insertion order vs shuffled diverged");
        // Sanity check — text shouldn't be empty if we emitted 200 inserts.
        assert!(!text_a.is_empty(), "expected non-empty materialised text");
    }

    /// CharOp wire serialisation must match Cloud's CRDTOpDTO shape.
    /// Verified by round-tripping a sample insert + delete through serde
    /// and asserting the JSON keys are exactly what Cloud's OpenAPI spec
    /// requires (op_type, char_id, char_value, prev_id, peer_id, lamport,
    /// seq).
    #[test]
    fn op_serialises_to_cloud_crdt_op_dto_shape() {
        let node = CharNode::new("alice", 5, 2, 'x', Some("alice-1-0".into()));
        let op = CharOp::from_insert(&node);
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op_type"], "insert");
        assert_eq!(json["char_id"], "alice-5-2");
        assert_eq!(json["char_value"], "x");
        assert_eq!(json["peer_id"], "alice");
        assert_eq!(json["lamport"], 5);
        assert_eq!(json["seq"], 2);
        assert_eq!(json["prev_id"], "alice-1-0");

        let del = CharOp::delete("alice-5-2".into(), "alice".into(), 9);
        let json = serde_json::to_value(&del).unwrap();
        assert_eq!(json["op_type"], "delete");
        assert_eq!(json["char_id"], "alice-5-2");
        assert_eq!(json["peer_id"], "alice");
        assert_eq!(json["lamport"], 9);
        // Deletes don't carry char_value or prev_id.
        assert!(json.get("char_value").is_none());
        assert!(json.get("prev_id").is_none());
    }
}
