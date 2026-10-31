import { Fragment, useSyncExternalStore, type ReactNode } from "react";
import { en } from "./en";

export type Lang = "fr" | "en";
const STORAGE_KEY = "vigie-lang";
const listeners = new Set<() => void>();

function detect(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "fr" || saved === "en") return saved;
  } catch {
    // stockage indisponible (fenêtre privée, etc.) : on retombe sur la langue du système
  }
  return typeof navigator !== "undefined" && navigator.language?.toLowerCase().startsWith("fr")
    ? "fr"
    : "en";
}

let current: Lang = detect();
syncDocumentLang();

function syncDocumentLang() {
  if (typeof document !== "undefined") document.documentElement.lang = current;
}

export const getLang = (): Lang => current;

export function setLang(lang: Lang): void {
  current = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // préférence non persistée : sans gravité
  }
  syncDocumentLang();
  listeners.forEach((f) => f());
}

/**
 * Traduit un texte écrit en français (la clé EST le texte français). En anglais, le texte est
 * cherché dans `en.ts` ; `{nom}` est remplacé par `vars.nom`. Un test vérifie que chaque clé
 * utilisée dans le code possède sa traduction.
 */
export function t(fr: string, vars?: Record<string, string | number>): string {
  const base = current === "fr" ? fr : (en[fr] ?? fr);
  return vars ? base.replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? "")) : base;
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => void listeners.delete(cb);
}

export function useLang(): [Lang, (l: Lang) => void] {
  const lang = useSyncExternalStore(subscribe, getLang, getLang);
  return [lang, setLang];
}

/** Remonte l'arbre quand la langue change : tous les `t()` sont réévalués. */
export function LangProvider({ children }: { children: ReactNode }) {
  const [lang] = useLang();
  return <Fragment key={lang}>{children}</Fragment>;
}
