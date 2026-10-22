/** Local resources only: no language detection, HTTP backend or shared mutable global instance. */
import { createInstance } from "i18next";
import ar from "./locales/ar.json" with { type: "json" };
import de from "./locales/de.json" with { type: "json" };
import en from "./locales/en.json" with { type: "json" };
import es from "./locales/es.json" with { type: "json" };
import fr from "./locales/fr.json" with { type: "json" };
import hi from "./locales/hi.json" with { type: "json" };
import it from "./locales/it.json" with { type: "json" };
import ja from "./locales/ja.json" with { type: "json" };
import ko from "./locales/ko.json" with { type: "json" };
import pt from "./locales/pt.json" with { type: "json" };
import ru from "./locales/ru.json" with { type: "json" };
import zhHans from "./locales/zh-Hans.json" with { type: "json" };
import zhHant from "./locales/zh-Hant.json" with { type: "json" };

export const LANGUAGES = [
  { id: "en", label: "English" },
  { id: "ar", label: "العربية" },
  { id: "zh-Hans", label: "简体中文" },
  { id: "zh-Hant", label: "繁體中文" },
  { id: "fr", label: "Français" },
  { id: "de", label: "Deutsch" },
  { id: "hi", label: "हिन्दी" },
  { id: "it", label: "Italiano" },
  { id: "ja", label: "日本語" },
  { id: "ko", label: "한국어" },
  { id: "pt", label: "Português" },
  { id: "ru", label: "Русский" },
  { id: "es", label: "Español" },
] as const;
export type Language = (typeof LANGUAGES)[number]["id"];
export const resources: Record<Language, Record<string, string>> = {
  en,
  ar,
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
  fr,
  de,
  hi,
  it,
  ja,
  ko,
  pt,
  ru,
  es,
};
export function createLocale(requested: string) {
  const language: Language = LANGUAGES.find((l) => l.id === requested)?.id ?? "en";
  const instance = createInstance();
  void instance.init({
    lng: language,
    fallbackLng: "en",
    load: "currentOnly",
    initAsync: false,
    supportedLngs: LANGUAGES.map((l) => l.id),
    keySeparator: false,
    nsSeparator: false,
    resources: Object.fromEntries(
      Object.entries(resources).map(([id, translation]) => [id, { translation }]),
    ),
    interpolation: { escapeValue: false },
  });
  return {
    language,
    direction: language === "ar" ? ("rtl" as const) : ("ltr" as const),
    t: (key: string, values?: Record<string, string | number>): string =>
      instance.t(key, { ...values, defaultValue: key }),
  };
}
export type Locale = ReturnType<typeof createLocale>;
