import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from 'react';
import { rpc } from './api';

// ─── Types ──────────────────────────────────────────────────────────────────

interface TranslationEntry {
  key: string;
  value: string;
}

interface TranslationsPayload {
  locale: string;
  entries: TranslationEntry[];
}

interface I18nContextValue {
  locale: string;
  t: (key: string) => string;
  t_fmt: (key: string, args: Record<string, string>) => string;
  loading: boolean;
}

// ─── Context ─────────────────────────────────────────────────────────────────

const I18nContext = createContext<I18nContextValue>({
  locale: 'en',
  t: (key: string) => key,
  t_fmt: (key: string) => key,
  loading: true,
});

const translationCache = new Map<string, Record<string, string>>();
const translationRequests = new Map<string, Promise<Record<string, string>>>();

async function loadTranslationMap(locale: string): Promise<Record<string, string>> {
  const cached = translationCache.get(locale);
  if (cached) return cached;

  const pending = translationRequests.get(locale);
  if (pending) return pending;

  const request = rpc<TranslationsPayload>('hub/get_translations', { locale })
    .then((result) => {
      const entryMap: Record<string, string> = {};
      for (const { key, value } of result.entries) {
        entryMap[key] = value;
      }
      translationCache.set(result.locale, entryMap);
      return entryMap;
    })
    .finally(() => {
      translationRequests.delete(locale);
    });

  translationRequests.set(locale, request);
  return request;
}

// ─── Hook ────────────────────────────────────────────────────────────────────

export function useTranslation(): I18nContextValue {
  return useContext(I18nContext);
}

// ─── Provider ────────────────────────────────────────────────────────────────

export function I18nProvider({
  locale,
  children,
}: {
  locale: string;
  children: React.ReactNode;
}) {
  const [map, setMap] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const localeRef = useRef<string | null>(null);

  const loadTranslations = useCallback(async (loc: string) => {
    localeRef.current = loc;
    setLoading(true);
    try {
      const entryMap = await loadTranslationMap(loc);
      if (localeRef.current !== loc) return;
      setMap(entryMap);
    } catch {
      // Fallback: use key as value when backend isn't available
      if (localeRef.current !== loc) return;
      setMap({});
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    loadTranslations(locale);
  }, [loadTranslations, locale]);

  const t = useCallback(
    (key: string): string => {
      return map[key] ?? key;
    },
    [map],
  );

  const t_fmt = useCallback(
    (key: string, args: Record<string, string>): string => {
      let result = map[key] ?? key;
      for (const [k, v] of Object.entries(args)) {
        result = result.replace(`{${k}}`, v);
      }
      return result;
    },
    [map],
  );

  return (
    <I18nContext.Provider value={{ locale, t, t_fmt, loading }}>
      {children}
    </I18nContext.Provider>
  );
}
