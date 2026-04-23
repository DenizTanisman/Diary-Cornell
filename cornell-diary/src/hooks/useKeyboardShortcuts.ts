import { useEffect } from 'react';

export interface KeyboardShortcutHandlers {
  onSave?: () => void;
  onPrevDay?: () => void;
  onNextDay?: () => void;
  onGoToDate?: () => void;
  onToday?: () => void;
}

export function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      if (e.key === 's') {
        e.preventDefault();
        handlers.onSave?.();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        handlers.onPrevDay?.();
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        handlers.onNextDay?.();
      } else if (e.key === 'g') {
        e.preventDefault();
        handlers.onGoToDate?.();
      } else if (e.key === 't') {
        e.preventDefault();
        handlers.onToday?.();
      }
    };

    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [handlers]);
}
