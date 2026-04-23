import { useCallback } from 'react';
import { useDiary } from '../../hooks/useDiary';
import { useDateNavigator } from '../../hooks/useDateNavigator';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { DateHeader } from './DateHeader';
import { MainNotesArea } from './MainNotesArea';
import { CueSection } from './CueSection';
import { SummaryBar } from './SummaryBar';
import { useT } from '../../locales';

function countWords(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).filter(Boolean).length;
}

export function CornellLayout() {
  const t = useT();
  const nav = useDateNavigator();
  const diary = useDiary({ date: nav.date });

  const handleSave = useCallback(() => {
    void diary.saveNow();
  }, [diary]);

  useKeyboardShortcuts({
    onSave: handleSave,
    onPrevDay: nav.goPrevDay,
    onNextDay: nav.goNextDay,
    onToday: nav.goToToday,
  });

  if (diary.isLoading) return <div className="app-loading">{t('app.loading')}</div>;

  if (diary.error) {
    return (
      <div className="app-error">
        <h2>{t('app.error')}</h2>
        <p>{diary.error.message}</p>
      </div>
    );
  }

  if (!diary.entry) return null;

  return (
    <div className="cornell-shell">
      <DateHeader
        date={nav.date}
        wordCount={countWords(diary.entry.diary)}
        isSaving={diary.isSaving}
        isDirty={diary.isDirty}
        onPrev={nav.goPrevDay}
        onNext={nav.goNextDay}
        onToday={nav.goToToday}
      />

      <div className="cornell-grid">
        <CueSection
          items={diary.entry.cueItems}
          onAdd={diary.addCueItem}
          onUpdate={diary.updateCueItem}
          onRemove={diary.removeCueItem}
        />
        <MainNotesArea value={diary.entry.diary} onChange={diary.updateDiary} />
      </div>

      <SummaryBar
        summary={diary.entry.summary}
        quote={diary.entry.quote}
        onSummaryChange={diary.updateSummary}
        onQuoteChange={diary.updateQuote}
      />
    </div>
  );
}
