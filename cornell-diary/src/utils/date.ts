import { format, parseISO, addDays, subDays, isValid } from 'date-fns';
import { tr } from 'date-fns/locale';

export function todayISO(): string {
  return toISODate(new Date());
}

export function toISODate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function parseISODate(date: string): Date {
  return parseISO(date);
}

export function isValidISODate(date: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return false;
  return isValid(parseISO(date));
}

export function formatTurkishLong(date: string): string {
  return format(parseISO(date), "d MMMM yyyy, EEEE", { locale: tr });
}

export function formatTurkishShort(date: string): string {
  return format(parseISO(date), 'd MMM yyyy', { locale: tr });
}

export function formatDayName(date: string): string {
  return format(parseISO(date), 'EEEE', { locale: tr });
}

export function addDaysISO(date: string, days: number): string {
  return toISODate(addDays(parseISO(date), days));
}

export function subDaysISO(date: string, days: number): string {
  return toISODate(subDays(parseISO(date), days));
}
