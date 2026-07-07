import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { Language } from "../lib/ipc.js";
import { useSettings } from "../hooks/useSettings.js";
import { en, translations, type TranslationKey } from "./translations.js";

/** Values interpolated into {placeholder} tokens. Numbers are stringified. */
export type TranslationParams = Record<string, string | number>;

/** Translator: resolves a key for the active language, interpolating params. */
export type TFunction = (id: TranslationKey, params?: TranslationParams) => string;

type TranslationContextValue = {
  language: Language;
  t: TFunction;
};

const DEFAULT_LANGUAGE: Language = "en";

/** Replaces every {name} token with the matching param, leaving unknown tokens. */
function interpolate(template: string, params?: TranslationParams): string {
  if (!params) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
}

/** Builds a translator for a language, falling back to en then to the raw id. */
export function createTranslator(language: Language): TFunction {
  const dictionary = translations[language] ?? en;
  return (id, params) => {
    const template = dictionary[id] ?? en[id] ?? id;
    return interpolate(template, params);
  };
}

const TranslationContext = createContext<TranslationContextValue>({
  language: DEFAULT_LANGUAGE,
  t: createTranslator(DEFAULT_LANGUAGE)
});

/**
 * Provides the active language (read from app settings) and a memoized `t` to
 * the tree. Mounted once in App so every screen shares the same translator.
 */
export function TranslationProvider({ children }: { children: ReactNode }) {
  const { data: settings } = useSettings();
  const language = settings?.language ?? DEFAULT_LANGUAGE;

  const value = useMemo<TranslationContextValue>(
    () => ({ language, t: createTranslator(language) }),
    [language]
  );

  return <TranslationContext.Provider value={value}>{children}</TranslationContext.Provider>;
}

/** Access the active translator and language from anywhere in the tree. */
export function useTranslation(): TranslationContextValue {
  return useContext(TranslationContext);
}
