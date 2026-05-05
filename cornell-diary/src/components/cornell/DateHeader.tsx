import type { ReactNode } from 'react';
import { useRef } from 'react';
import { formatTurkishLong, isValidISODate } from '../../utils/date';
import { DateNavigator } from '../common/DateNavigator';
import { SaveIndicator } from '../common/SaveIndicator';
import { useT } from '../../locales';

interface Props {
  date: string;
  wordCount: number;
  isSaving: boolean;
  isDirty: boolean;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onPickDate: (iso: string) => void;
  /** Optional slot rendered next to the date — FAZ 3 uses it for the
   *  presence badge. Older callers may omit it. */
  afterTitle?: ReactNode;
}

export function DateHeader({
  date,
  wordCount,
  isSaving,
  isDirty,
  onPrev,
  onNext,
  onToday,
  onPickDate,
  afterTitle,
}: Props) {
  const t = useT();
  const dateInputRef = useRef<HTMLInputElement>(null);

  const openPicker = () => {
    const el = dateInputRef.current;
    if (!el) return;
    // Modern WebKit / Chromium expose showPicker(); fall back to a
    // synthetic click so older WebViews still surface the OS picker.
    if (typeof el.showPicker === 'function') {
      try {
        el.showPicker();
        return;
      } catch {
        // some platforms throw if not user-initiated — fall through.
      }
    }
    el.focus();
    el.click();
  };

  return (
    <header className="cornell-header">
      <DateNavigator onPrev={onPrev} onNext={onNext} onToday={onToday} />
      <button
        type="button"
        className="cornell-header__date cornell-header__date--button"
        aria-label={`${formatTurkishLong(date)} — tarih seç`}
        onClick={openPicker}
      >
        <span className="cornell-header__date-text">{formatTurkishLong(date)}</span>
        {afterTitle}
        <input
          ref={dateInputRef}
          type="date"
          value={date}
          onChange={(e) => {
            const next = e.target.value;
            if (isValidISODate(next)) onPickDate(next);
          }}
          className="cornell-header__date-picker"
          tabIndex={-1}
          aria-hidden="true"
        />
      </button>
      <div className="cornell-header__right">
        <span className="cornell-header__counter">
          {t('diary.wordCount', { count: wordCount })}
        </span>
        <SaveIndicator isSaving={isSaving} isDirty={isDirty} />
      </div>
    </header>
  );
}
