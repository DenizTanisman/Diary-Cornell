import type { QRChunkFrame } from './qrChunker';

export interface AssemblerState {
  sessionId: string | null;
  totalFrames: number;
  received: Map<number, string>;
}

export function createAssembler(): AssemblerState {
  return { sessionId: null, totalFrames: 0, received: new Map() };
}

export function addFrame(
  state: AssemblerState,
  frame: QRChunkFrame,
): { accepted: boolean; reason?: string } {
  if (!state.sessionId) {
    state.sessionId = frame.sessionId;
    state.totalFrames = frame.totalFrames;
  } else if (state.sessionId !== frame.sessionId) {
    return { accepted: false, reason: 'session_mismatch' };
  } else if (state.totalFrames !== frame.totalFrames) {
    return { accepted: false, reason: 'total_mismatch' };
  }
  state.received.set(frame.frameNum, frame.payload);
  return { accepted: true };
}

export function isComplete(state: AssemblerState): boolean {
  if (!state.sessionId || state.totalFrames === 0) return false;
  for (let i = 1; i <= state.totalFrames; i++) {
    if (!state.received.has(i)) return false;
  }
  return true;
}

export function progress(state: AssemblerState): { received: number; total: number } {
  return { received: state.received.size, total: state.totalFrames };
}

export function assemble(state: AssemblerState): string {
  if (!isComplete(state)) {
    throw new Error('Assembler is not complete — missing frames.');
  }
  const parts: string[] = [];
  for (let i = 1; i <= state.totalFrames; i++) {
    parts.push(state.received.get(i) ?? '');
  }
  const base64 = parts.join('');
  return decodeURIComponent(escape(atob(base64)));
}
