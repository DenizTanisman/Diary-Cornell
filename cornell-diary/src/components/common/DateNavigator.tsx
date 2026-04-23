import { useT } from '../../locales';

interface Props {
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  disableNext?: boolean;
}

export function DateNavigator({ onPrev, onNext, onToday, disableNext }: Props) {
  const t = useT();
  return (
    <div className="cornell-header__left" role="navigation" aria-label="date navigation">
      <button className="cornell-nav-button" onClick={onPrev} aria-label={t('nav.prev')}>
        ←
      </button>
      <button className="cornell-nav-button" onClick={onToday}>
        {t('nav.today')}
      </button>
      <button
        className="cornell-nav-button"
        onClick={onNext}
        aria-label={t('nav.next')}
        disabled={disableNext}
      >
        →
      </button>
    </div>
  );
}
