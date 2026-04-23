import { useT } from '../../locales';

interface Props {
  summary: string;
  quote: string;
  onSummaryChange: (v: string) => void;
  onQuoteChange: (v: string) => void;
}

export function SummaryBar({ summary, quote, onSummaryChange, onQuoteChange }: Props) {
  const t = useT();
  return (
    <footer className="cornell-summary" aria-label="summary bar">
      <label className="cornell-summary__field">
        <span className="cornell-summary__label">{t('diary.summaryLabel')}</span>
        <input
          className="cornell-summary__input"
          type="text"
          value={summary}
          placeholder={t('diary.summaryPlaceholder')}
          onChange={(e) => onSummaryChange(e.target.value)}
        />
      </label>
      <label className="cornell-summary__field">
        <span className="cornell-summary__label">{t('diary.quoteLabel')}</span>
        <input
          className="cornell-summary__input"
          type="text"
          value={quote}
          placeholder={t('diary.quotePlaceholder')}
          onChange={(e) => onQuoteChange(e.target.value)}
        />
      </label>
    </footer>
  );
}
