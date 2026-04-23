import type { CueItem as CueItemType } from '../../types/diary';
import { useT } from '../../locales';

interface Props {
  item: CueItemType;
  onUpdate: (changes: Partial<Omit<CueItemType, 'position'>>) => void;
  onRemove: () => void;
}

export function CueItem({ item, onUpdate, onRemove }: Props) {
  const t = useT();
  return (
    <article className="cue-item" aria-label={`cue item ${item.position}`}>
      <header className="cue-item__header">
        <input
          className="cue-item__title"
          value={item.title}
          onChange={(e) => onUpdate({ title: e.target.value })}
          aria-label={`cue title ${item.position}`}
        />
        <button
          className="cue-item__remove"
          onClick={onRemove}
          aria-label={t('cue.remove')}
          title={t('cue.remove')}
        >
          ✕
        </button>
      </header>
      <textarea
        className="cue-item__content"
        value={item.content}
        placeholder={t('cue.contentPlaceholder')}
        onChange={(e) => onUpdate({ content: e.target.value })}
      />
    </article>
  );
}
