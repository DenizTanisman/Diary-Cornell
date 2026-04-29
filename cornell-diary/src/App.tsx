import { Navigate, Route, Routes } from 'react-router-dom';
import { DiaryPage } from './pages/DiaryPage';
import { ArchivePage } from './pages/ArchivePage';
import { SettingsPage } from './pages/SettingsPage';
import { SyncPage } from './pages/SyncPage';
import { NotFoundPage } from './pages/NotFoundPage';
import { MigrationOnboarding } from './pages/MigrationOnboarding';
import { AppToolbar } from './components/common/AppToolbar';
import { useTheme } from './hooks/useTheme';
import { todayISO } from './utils/date';

export default function App() {
  useTheme();
  return (
    <>
      <AppToolbar />
      <Routes>
        <Route path="/" element={<Navigate to={`/diary/${todayISO()}`} replace />} />
        <Route path="/diary" element={<Navigate to={`/diary/${todayISO()}`} replace />} />
        <Route path="/diary/:date" element={<DiaryPage />} />
        <Route path="/archive" element={<ArchivePage />} />
        <Route path="/sync" element={<SyncPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/migration" element={<MigrationOnboarding />} />
        <Route path="*" element={<NotFoundPage />} />
      </Routes>
    </>
  );
}
