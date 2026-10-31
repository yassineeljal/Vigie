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
        return "Mot de passe ou clé de récupération incorrect.";
      case "weak_password":
        return "Mot de passe trop faible.";
      case "invalid_recovery_key":
        return "Clé de récupération invalide.";
      default:
        return e.error.message;
    }
  }
  return "Une erreur inattendue est survenue.";
}
