/**
 * CRDT wire shapes — mirror `src-tauri/src/crdt/{operations,ws_proto}.rs`.
 *
 * Frontend doesn't generate ops directly in FAZ 3.3 — it sends new full
 * text via `apply_local_text`, and Rust diffs / emits CharOps. We still
 * declare the inbound shape because `crdt:text-updated` and friends
 * carry it over the Tauri event bus.
 */

export type CharOp =
  | {
      op_type: 'insert';
      char_id: string;
      char_value: string;
      peer_id: string;
      lamport: number;
      seq: number;
      prev_id: string | null;
    }
  | {
      op_type: 'delete';
      char_id: string;
      peer_id: string;
      lamport: number;
    };

export interface CrdtTextUpdated {
  entry_date: string;
  field: string;
  text: string;
  from_peer: string | null;
}

export interface CrdtPresence {
  peers: string[];
}

export interface CrdtSnapshotUpdated {
  entry_date: string;
  version: number;
}
