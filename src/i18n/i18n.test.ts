import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { getLang, setLang, t } from "./index";
import { en } from "./en";
import { ACTION_LABELS, FACTOR_LABELS } from "../lib/factors";
import { CRITICALITY_LABELS, IDENTITY_LABELS, MFA_LABELS, STATUS_LABELS } from "../lib/labels";

const SRC = join(__dirname, "..");

function sources(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name !== "i18n") sources(p, out);
    } else if (/\.(ts|tsx)$/.test(name) && !/\.test\.|ipc\.ts$/.test(name)) out.push(p);
  }
  return out;
}

/** Clés françaises utilisées dans le code : appels t("…"), libellés de constantes. */
function usedKeys(): Set<string> {
  const keys = new Set<string>();
  const lit = (s: string) => JSON.parse(`"${s}"`) as string;
  for (const file of sources(SRC)) {
    const text = readFileSync(file, "utf8");
    for (const m of text.matchAll(/\bt\(\s*"((?:[^"\\]|\\.)*)"/g)) keys.add(lit(m[1]));
    for (const m of text.matchAll(/\blabel:\s*"((?:[^"\\]|\\.)*)"/g)) keys.add(lit(m[1]));
    for (const m of text.matchAll(
      /\b(?:bitwarden_csv|bitwarden_json|apple_passwords|browser_csv|generic_csv):\s*"((?:[^"\\]|\\.)*)"/g,
    ))
      keys.add(lit(m[1]));
    for (const m of text.matchAll(/\b(?:title|body):\s*"((?:[^"\\]|\\.)*)"/g)) keys.add(lit(m[1]));
    for (const m of text.matchAll(/\["\w+",\s*"((?:[^"\\]|\\.)*)"\]/g)) keys.add(lit(m[1]));
    for (const m of text.matchAll(/^(?:const|export const) (?:STRENGTH|LABELS) = \[([^\]]*)\]/gm)) {
      for (const s of m[1].matchAll(/"((?:[^"\\]|\\.)*)"/g)) keys.add(lit(s[1]));
    }
  }
  for (const map of [
    ACTION_LABELS,
    FACTOR_LABELS,
    CRITICALITY_LABELS,
    IDENTITY_LABELS,
    MFA_LABELS,
    STATUS_LABELS,
  ]) {
    Object.values(map).forEach((v) => keys.add(v));
  }
  keys.delete("");
  return keys;
}

describe("i18n", () => {
  it("chaque texte français utilisé dans le code a sa traduction anglaise", () => {
    const missing = [...usedKeys()].filter((k) => !(k in en));
    expect(missing).toEqual([]);
  });

  it("aucune traduction orpheline ni identique à la source (sauf noms propres)", () => {
    const used = usedKeys();
    const orphans = Object.keys(en).filter((k) => !used.has(k) && !CORE_KEYS.has(k));
    expect(orphans).toEqual([]);
  });

  it("les variables {x} sont conservées dans la traduction", () => {
    const vars = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const [fr, en_] of Object.entries(en)) expect(vars(en_), fr).toEqual(vars(fr));
  });

  it("bascule de langue et interpolation", () => {
    setLang("en");
    expect(getLang()).toBe("en");
    expect(t("Annuler")).toBe("Cancel");
    expect(t("Top {length} des actions", { length: 3 })).toBe("Top 3 actions");
    expect(t("texte absent du dictionnaire")).toBe("texte absent du dictionnaire");
    setLang("fr");
    expect(t("Annuler")).toBe("Annuler");
  });
});

/** Messages du cœur Rust, traduits dans result.ts (pas appelés via t("…") dans le code). */
const CORE_KEYS = new Set<string>(
  readFileSync(join(SRC, "i18n", "core-messages.txt"), "utf8")
    .split("\n")
    .filter(Boolean),
);
