import tr from './tr.json';
import en from './en.json';
import type { Language } from '../types/settings';
import { useSettingsStore } from '../stores/settingsStore';

type Dictionary = typeof tr;

const dictionaries: Record<Language, Dictionary> = { tr, en };

type DotNested<T, K extends string = ''> = T extends string
  ? K
  : {
      [P in keyof T & string]: DotNested<T[P], K extends '' ? P : `${K}.${P}`>;
    }[keyof T & string];

type TranslationKey = DotNested<Dictionary>;

function resolve(lang: Language, key: string): string {
  const parts = key.split('.');
  let node: unknown = dictionaries[lang];
  for (const p of parts) {
    if (node && typeof node === 'object' && p in (node as object)) {
      node = (node as Record<string, unknown>)[p];
    } else {
      return key;
    }
  }
  return typeof node === 'string' ? node : key;
}

function applyVars(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, name) => String(vars[name] ?? ''));
}

export function t(key: TranslationKey, vars?: Record<string, string | number>): string {
  const lang = useSettingsStore.getState().language;
  return applyVars(resolve(lang, key), vars);
}

export function useT(): (key: TranslationKey, vars?: Record<string, string | number>) => string {
  const lang = useSettingsStore((s) => s.language);
  return (key, vars) => applyVars(resolve(lang, key), vars);
}
