import { Link } from 'react-router-dom';
import { todayISO } from '../utils/date';

export function NotFoundPage() {
  return (
    <div className="page-container">
      <h1>Sayfa bulunamadı</h1>
      <p>
        <Link to={`/diary/${todayISO()}`}>Bugüne dön</Link>
      </p>
    </div>
  );
}
