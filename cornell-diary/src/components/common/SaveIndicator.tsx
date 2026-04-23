import clsx from 'clsx';
import { useT } from '../../locales';

interface Props {
  isSaving: boolean;
  isDirty: boolean;
}

export function SaveIndicator({ isSaving, isDirty }: Props) {
  const t = useT();
  const state = isSaving ? 'saving' : isDirty ? 'dirty' : 'saved';
  const label = isSaving ? t('save.saving') : isDirty ? t('save.dirty') : t('save.saved');
  return (
    <span
      className={clsx('save-indicator', `save-indicator--${state}`)}
      aria-live="polite"
      role="status"
    >
      <span className="save-indicator__dot" aria-hidden="true" />
      <span>{label}</span>
    </span>
  );
}
