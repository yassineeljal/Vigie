import { t } from "../i18n";
import type { CommandError } from "./ipc";

type IpcResult<T> = { status: "ok"; data: T } | { status: "error"; error: CommandError };

export class IpcError extends Error {
  constructor(public readonly error: CommandError) {
    super(error.message);
  }
}

/** Convertit un résultat tauri-specta en promesse qui rejette avec `IpcError`. */
export async function unwrap<T>(p: Promise<IpcResult<T>>): Promise<T> {
  const r = await p;
  if (r.status === "error") throw new IpcError(r.error);
  return r.data;
}

export function errorMessage(e: unknown): string {
  if (e instanceof IpcError) {
    switch (e.error.kind) {
      case "bad_credentials":
        return t("Mot de passe ou clé de récupération incorrect.");
      case "weak_password":
        return t("Mot de passe trop faible.");
      case "invalid_recovery_key":
        return t("Clé de récupération invalide.");
      default:
        return coreMessage(e.error.message);
    }
  }
  return t("Une erreur inattendue est survenue.");
}

/** Messages d'erreur du cœur Rust (en français) : traduits ici, avec leurs parties variables. */
const DYNAMIC: [RegExp, string][] = [
  [/^domaine invalide : (.*)$/s, "domaine invalide : {0}"],
  [/^données invalides : (.*)$/s, "données invalides : {0}"],
  [/^format non reconnu : (.*)$/s, "format non reconnu : {0}"],
  [/^lecture impossible : (.*)$/s, "lecture impossible : {0}"],
  [/^écriture impossible : (.*)$/s, "écriture impossible : {0}"],
  [/^ouverture impossible : (.*)$/s, "ouverture impossible : {0}"],
  [/^réponse inattendue du service \(HTTP (\d+)\)$/, "réponse inattendue du service (HTTP {0})"],
  [
    /^mot de passe maître trop faible \(score zxcvbn (\d)\/4, minimum 3\)$/,
    "mot de passe maître trop faible (score zxcvbn {0}/4, minimum 3)",
  ],
];

export function coreMessage(message: string): string {
  for (const [re, key] of DYNAMIC) {
    const m = re.exec(message);
    if (m) return t(key).replace("{0}", m[1]);
  }
  return t(message);
}
