import type { ReactNode } from 'react';
import { formatTurkishLong } from '../../utils/date';
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
  afterTitle,
}: Props) {
  const t = useT();
  return (
    <header className="cornell-header">
      <DateNavigator onPrev={onPrev} onNext={onNext} onToday={onToday} />
      <div className="cornell-header__date" aria-label={formatTurkishLong(date)}>
        {formatTurkishLong(date)}
        {afterTitle}
      </div>
      <div className="cornell-header__right">
        <span className="cornell-header__counter">
          {t('diary.wordCount', { count: wordCount })}
        </span>
        <SaveIndicator isSaving={isSaving} isDirty={isDirty} />
      </div>
    </header>
  );
}
