import { useState } from "react";
import { commands, type ExportFormat, type SavedFile } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { t } from "../../i18n";
import { Button, ErrorText } from "../../components/ui";
import { useQueryClient } from "@tanstack/react-query";
import { VAULT_KEY } from "../../lib/useVault";

/** Export de l'inventaire (sans secrets) et sauvegarde chiffrée du coffre (US-60, US-61). */
export function DataSection() {
  const qc = useQueryClient();
  const [notes, setNotes] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string }>();
  const [restorePw, setRestorePw] = useState("");

  async function run(task: () => Promise<SavedFile | null>, done: (f: SavedFile) => string) {
    setMsg(undefined);
    try {
      const f = await task();
      if (f) setMsg({ ok: true, text: done(f) });
    } catch (e) {
      setMsg({ ok: false, text: errorMessage(e) });
    }
  }

  const exportAs = (format: ExportFormat) =>
    run(
      () => unwrap(commands.exportInventory(format, { include_notes: notes })),
      (f) =>
        t("Inventaire exporté dans {path}. Il contient tes courriels : garde-le en lieu sûr.", {
          path: f.path,
        }),
    );

  async function restore(e: React.FormEvent) {
    e.preventDefault();
    setMsg(undefined);
    try {
      const restored = await unwrap(commands.restoreVaultBackup(restorePw));
      setRestorePw("");
      if (restored) await qc.invalidateQueries({ queryKey: VAULT_KEY });
    } catch (err) {
      setRestorePw("");
      setMsg({ ok: false, text: errorMessage(err) });
    }
  }

  return (
    <div className="max-w-xl space-y-6">
      <div className="space-y-2">
        <h3 className="text-sm font-medium text-ink">{t("Exporter l'inventaire")}</h3>
        <p className="text-xs text-muted">
          {t(
            "Aucun mot de passe n'est exporté (Vigie n'en stocke pas). Les cellules CSV dangereuses pour un tableur sont neutralisées.",
          )}
        </p>
        <label className="flex items-center gap-2 text-sm text-ink">
          <input type="checkbox" checked={notes} onChange={(e) => setNotes(e.target.checked)} />
          {t("Inclure les notes")}
        </label>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={() => void exportAs("json")}>
            {t("Exporter en JSON")}
          </Button>
          <Button variant="ghost" onClick={() => void exportAs("csv")}>
            {t("Exporter en CSV")}
          </Button>
        </div>
      </div>

      <div className="space-y-2">
        <h3 className="text-sm font-medium text-ink">{t("Sauvegarde chiffrée")}</h3>
        <p className="text-xs text-muted">
          {t(
            "Le fichier .vigiebak reste chiffré : il s'ouvre avec ton mot de passe maître (ou ta clé de récupération).",
          )}
        </p>
        <Button
          variant="ghost"
          onClick={() =>
            void run(
              () => unwrap(commands.backupVault()),
              (f) => t("Sauvegarde créée : {path}", { path: f.path }),
            )
          }
        >
          {t("Créer une sauvegarde")}
        </Button>
      </div>

      <form onSubmit={restore} className="space-y-2">
        <h3 className="text-sm font-medium text-ink">{t("Restaurer une sauvegarde")}</h3>
        <p className="text-xs text-muted">
          {t(
            "Remplace le coffre actuel (conservé à côté, renommé) et te redemande le mot de passe de la sauvegarde.",
          )}
        </p>
        <label className="block space-y-1 text-sm">
          <span className="text-ink">{t("Mot de passe maître de la sauvegarde")}</span>
          <input
            type="password"
            autoComplete="off"
            value={restorePw}
            onChange={(e) => setRestorePw(e.target.value)}
            className="w-full rounded-md border border-line bg-surface px-3 py-2 text-ink"
          />
        </label>
        <Button type="submit" variant="ghost" disabled={!restorePw}>
          {t("Choisir le fichier et restaurer")}
        </Button>
      </form>

      {msg &&
        (msg.ok ? (
          <p role="status" className="text-sm text-safe">
            {msg.text}
          </p>
        ) : (
          <ErrorText>{msg.text}</ErrorText>
        ))}
    </div>
  );
}
