/**
 * Main notes textarea — gateway to FAZ 3 live editing.
 *
 * Two modes share the same DOM element:
 *  - **Solo mode** (default): the textarea is fully controlled by the
 *    parent's `value`/`onChange` (i.e. the existing useDiary debounced
 *    save path). No WS traffic.
 *  - **Live mode**: when `crdtMode` is true the on-change handler routes
 *    every keystroke through `onLocalText` (Tauri → Rust diff → WS).
 *    The displayed value is `value` — but `value` is now driven by
 *    Rust's CrdtDocument (via `useCrdtChannel`), not by useDiary.
 *
 * Cursor preservation: a controlled <textarea> loses its caret position
 * when `value` updates from a *remote* op (the React reconciliation
 * runs after our setState, so the selection collapses). We capture the
 * caret before the update and restore it (clamped to the new length)
 * in a `useLayoutEffect`.
 */
import { useLayoutEffect, useRef, type ChangeEvent } from 'react';
import { useT } from '../../locales';

interface Props {
  /** Current text. In solo mode comes from `useDiary`; in live mode
   *  from `useCrdtChannel`. */
  value: string;
  /** Solo-mode change handler. */
  onChange: (value: string) => void;
  /** True when at least one other peer is subscribed. Flips the
   *  on-change routing. */
  crdtMode?: boolean;
  /** Live-mode change handler — invoked instead of `onChange` when
   *  `crdtMode` is true. */
  onLocalText?: (next: string) => void;
}

export function MainNotesArea({
  value,
  onChange,
  crdtMode = false,
  onLocalText,
}: Props) {
  const t = useT();
  const ref = useRef<HTMLTextAreaElement>(null);
  // Snapshot the caret on every keystroke so the next remote update
  // can land it back where the user was typing. We don't store
  // selection on every render — only when the user actually moves it.
  const lastSelection = useRef<{ start: number; end: number } | null>(null);

  function rememberSelection() {
    const el = ref.current;
    if (!el) return;
    lastSelection.current = { start: el.selectionStart, end: el.selectionEnd };
  }

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (document.activeElement !== el) return; // user is elsewhere — leave it
    const sel = lastSelection.current;
    if (!sel) return;
    const cap = value.length;
    el.setSelectionRange(Math.min(sel.start, cap), Math.min(sel.end, cap));
  }, [value]);

  function handleChange(e: ChangeEvent<HTMLTextAreaElement>) {
    rememberSelection();
    if (crdtMode && onLocalText) {
      onLocalText(e.target.value);
    } else {
      onChange(e.target.value);
    }
  }

  return (
    <section className="cornell-main" aria-label="main notes">
      <textarea
        ref={ref}
        className="cornell-main__textarea"
        value={value}
        placeholder={t('diary.mainPlaceholder')}
        onChange={handleChange}
        onSelect={rememberSelection}
        onKeyUp={rememberSelection}
        onMouseUp={rememberSelection}
        spellCheck
        autoFocus
        data-crdt-mode={crdtMode ? 'live' : 'solo'}
      />
    </section>
  );
}
