import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";

type Messages = Record<string, string>;

const packs: Record<string, Messages> = {
  en,
  "zh-CN": zhCN,
};

let currentLanguage = "en";

export function setLanguage(language: string): void {
  currentLanguage = packs[language] ? language : "en";
  document.documentElement.lang = currentLanguage;
}

export function language(): string {
  return currentLanguage;
}

export function availableLanguages(): string[] {
  return Object.keys(packs);
}

export function t(key: string, values: Record<string, string | number> = {}): string {
  const template = packs[currentLanguage]?.[key] ?? packs.en[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? `{${name}}`));
}
