import { useState, type FormEvent } from 'react';
import type { CueItem as CueItemType } from '../../types/diary';
import { MAX_CUE_ITEMS } from '../../types/diary';
import { CueItem } from './CueItem';
import { useT } from '../../locales';

interface Props {
  items: CueItemType[];
  onAdd: (title: string) => void;
  onUpdate: (position: number, changes: Partial<Omit<CueItemType, 'position'>>) => void;
  onRemove: (position: number) => void;
}

export function CueSection({ items, onAdd, onUpdate, onRemove }: Props) {
  const t = useT();
  const [draft, setDraft] = useState('');
  const isFull = items.length >= MAX_CUE_ITEMS;

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const title = draft.trim();
    if (!title || isFull) return;
    onAdd(title);
    setDraft('');
  };

  return (
    <aside className="cornell-cue" aria-label="cue section">
      {items
        .slice()
        .sort((a, b) => a.position - b.position)
        .map((item) => (
          <CueItem
            key={item.position}
            item={item}
            onUpdate={(changes) => onUpdate(item.position, changes)}
            onRemove={() => onRemove(item.position)}
          />
        ))}

      <form className="cue-add" onSubmit={handleSubmit}>
        <input
          className="cue-add__input"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={t('cue.addPlaceholder')}
          disabled={isFull}
          aria-label={t('cue.add')}
        />
        <button
          className="cue-add__button"
          type="submit"
          disabled={isFull || draft.trim().length === 0}
        >
          {t('cue.addButton')}
        </button>
      </form>

      {isFull ? (
        <p className="empty-state">{t('cue.maxReached', { max: MAX_CUE_ITEMS })}</p>
      ) : null}
    </aside>
  );
}
