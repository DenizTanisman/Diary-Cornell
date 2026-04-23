import { create } from 'zustand';
import type { SyncResult } from '../types/sync';

type SyncDialog = 'none' | 'export' | 'import' | 'qr-send' | 'qr-scan';

interface SyncState {
  dialog: SyncDialog;
  lastResult: SyncResult | null;
  openDialog: (d: SyncDialog) => void;
  closeDialog: () => void;
  setLastResult: (r: SyncResult | null) => void;
}

export const useSyncStore = create<SyncState>((set) => ({
  dialog: 'none',
  lastResult: null,
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: 'none' }),
  setLastResult: (lastResult) => set({ lastResult }),
}));
