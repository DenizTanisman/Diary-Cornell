import { useT } from '../../locales';

interface Props {
  value: string;
  onChange: (value: string) => void;
}

export function MainNotesArea({ value, onChange }: Props) {
  const t = useT();
  return (
    <section className="cornell-main" aria-label="main notes">
      <textarea
        className="cornell-main__textarea"
        value={value}
        placeholder={t('diary.mainPlaceholder')}
        onChange={(e) => onChange(e.target.value)}
        spellCheck
        autoFocus
      />
    </section>
  );
}
