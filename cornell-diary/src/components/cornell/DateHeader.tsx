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
}

export function DateHeader({ date, wordCount, isSaving, isDirty, onPrev, onNext, onToday }: Props) {
  const t = useT();
  return (
    <header className="cornell-header">
      <DateNavigator onPrev={onPrev} onNext={onNext} onToday={onToday} />
      <div className="cornell-header__date" aria-label={formatTurkishLong(date)}>
        {formatTurkishLong(date)}
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
