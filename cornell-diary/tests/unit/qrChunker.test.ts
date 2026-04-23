import { describe, expect, it } from 'vitest';
import { chunkJSONForQR, decodeFrame, encodeFrame } from '../../src/sync/qrChunker';
import {
  addFrame,
  assemble,
  createAssembler,
  isComplete,
} from '../../src/sync/qrAssembler';

describe('QR chunker/assembler', () => {
  it('round-trips JSON through chunking and assembly', () => {
    const payload = JSON.stringify({ msg: 'x'.repeat(5000), list: [1, 2, 3] });
    const frames = chunkJSONForQR(payload, 1000);
    expect(frames.length).toBeGreaterThan(1);
    const session = frames[0].sessionId;

    const state = createAssembler();
    for (const f of frames) {
      expect(f.sessionId).toBe(session);
      const accepted = addFrame(state, f);
      expect(accepted.accepted).toBe(true);
    }

    expect(isComplete(state)).toBe(true);
    const restored = assemble(state);
    expect(restored).toBe(payload);
  });

  it('encodes and decodes the wire format', () => {
    const [frame] = chunkJSONForQR('{"a":1}');
    const wire = encodeFrame(frame);
    expect(wire.startsWith('CDIA1|1/1|')).toBe(true);
    const decoded = decodeFrame(wire);
    expect(decoded).toMatchObject({
      sessionId: frame.sessionId,
      frameNum: 1,
      totalFrames: 1,
      payload: frame.payload,
    });
  });

  it('rejects frames from a different session', () => {
    const frames = chunkJSONForQR('hello');
    const state = createAssembler();
    addFrame(state, frames[0]);
    const intruder = { ...frames[0], sessionId: 'OTHER' };
    const accepted = addFrame(state, intruder);
    expect(accepted.accepted).toBe(false);
  });

  it('returns null for malformed wire data', () => {
    expect(decodeFrame('INVALID')).toBeNull();
  });
});
