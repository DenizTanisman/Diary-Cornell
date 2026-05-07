import { useEffect } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import { DiaryPage } from './pages/DiaryPage';
import { ArchivePage } from './pages/ArchivePage';
import { SettingsPage } from './pages/SettingsPage';
import { SyncPage } from './pages/SyncPage';
import { NotFoundPage } from './pages/NotFoundPage';
import { AppToolbar } from './components/common/AppToolbar';
import { useTheme } from './hooks/useTheme';
import { usePlatform } from './hooks/usePlatform';
import { todayISO } from './utils/date';

export default function App() {
  useTheme();
  const { platform } = usePlatform();
  // Surface the platform to CSS so layouts that need to compensate for
  // host-OS quirks (e.g. Android WebView not translating window insets
  // to env(safe-area-inset-*)) can target a single attribute selector.
  useEffect(() => {
    document.body.setAttribute('data-platform', platform);
  }, [platform]);
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
        <Route path="*" element={<NotFoundPage />} />
      </Routes>
    </>
  );
}
