import { useCallback } from 'react';
import { useDiary } from '../../hooks/useDiary';
import { useDateNavigator } from '../../hooks/useDateNavigator';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { useCrdtChannel } from '../../hooks/useCrdtChannel';
import { useSyncStatus } from '../../hooks/useSyncStatus';
import { DateHeader } from './DateHeader';
import { MainNotesArea } from './MainNotesArea';
import { CueSection } from './CueSection';
import { SummaryBar } from './SummaryBar';
import { PresenceBadge } from './PresenceBadge';
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
  // Cloud sync must be online before the WS pipe is worth opening.
  // The status hook polls every 5s; when sync is enabled & online,
  // crdtChannel boots up.
  const { status } = useSyncStatus();
  const liveEnabled = (status?.enabled ?? false) && (status?.online ?? false);
  const crdt = useCrdtChannel({
    entryDate: nav.date,
    fieldName: 'diary',
    seedText: diary.entry?.diary ?? '',
    enabled: liveEnabled,
  });

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
        wordCount={countWords(crdt.crdtMode ? crdt.text : diary.entry.diary)}
        isSaving={diary.isSaving}
        isDirty={diary.isDirty}
        onPrev={nav.goPrevDay}
        onNext={nav.goNextDay}
        onToday={nav.goToToday}
        afterTitle={<PresenceBadge peers={crdt.peers} localPeerId={null} />}
      />

      <div className="cornell-grid">
        <CueSection
          items={diary.entry.cueItems}
          onAdd={diary.addCueItem}
          onUpdate={diary.updateCueItem}
          onRemove={diary.removeCueItem}
        />
        <MainNotesArea
          value={crdt.crdtMode ? crdt.text : diary.entry.diary}
          onChange={diary.updateDiary}
          crdtMode={crdt.crdtMode}
          onLocalText={(next) => {
            // Mirror the live text into useDiary so the
            // debounced Postgres save still snapshots the
            // result. Rust handles the actual op broadcast.
            diary.updateDiary(next);
            void crdt.applyLocalText(next);
          }}
        />
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
