import { useCallback, useMemo } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { addDaysISO, isValidISODate, subDaysISO, todayISO } from '../utils/date';

export interface UseDateNavigatorReturn {
  date: string;
  isToday: boolean;
  goToDate: (date: string) => void;
  goToToday: () => void;
  goPrevDay: () => void;
  goNextDay: () => void;
}

export function useDateNavigator(): UseDateNavigatorReturn {
  const navigate = useNavigate();
  const params = useParams<{ date?: string }>();

  const date = useMemo(() => {
    const fromUrl = params.date;
    if (fromUrl && isValidISODate(fromUrl)) return fromUrl;
    return todayISO();
  }, [params.date]);

  const goToDate = useCallback(
    (d: string) => {
      if (!isValidISODate(d)) return;
      navigate(`/diary/${d}`);
    },
    [navigate],
  );

  const goToToday = useCallback(() => goToDate(todayISO()), [goToDate]);
  const goPrevDay = useCallback(() => goToDate(subDaysISO(date, 1)), [date, goToDate]);
  const goNextDay = useCallback(() => goToDate(addDaysISO(date, 1)), [date, goToDate]);

  return {
    date,
    isToday: date === todayISO(),
    goToDate,
    goToToday,
    goPrevDay,
    goNextDay,
  };
}
