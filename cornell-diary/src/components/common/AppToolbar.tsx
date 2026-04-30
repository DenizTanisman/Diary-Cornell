import { NavLink } from 'react-router-dom';
import clsx from 'clsx';
import { useT } from '../../locales';
import { todayISO } from '../../utils/date';
import { SyncIndicator } from '../sync/SyncIndicator';

export function AppToolbar() {
  const t = useT();
  const today = todayISO();
  const item = (path: string, label: string) => (
    <NavLink
      to={path}
      className={({ isActive }) => clsx('toolbar__link', isActive && 'toolbar__link--active')}
    >
      {label}
    </NavLink>
  );
  return (
    <nav className="toolbar" aria-label="main navigation">
      {item(`/diary/${today}`, t('nav.today'))}
      {item('/archive', t('nav.archive'))}
      {item('/sync', t('nav.sync'))}
      {item('/settings', t('nav.settings'))}
      <span className="toolbar__spacer" />
      <SyncIndicator />
      <span className="cornell-header__counter" aria-hidden="true">
        Cornell Diary
      </span>
    </nav>
  );
}
