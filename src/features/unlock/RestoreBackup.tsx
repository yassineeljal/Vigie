import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { VAULT_KEY } from "../../lib/useVault";
import { t } from "../../i18n";
import { Button, Card, ErrorText, Field } from "../../components/ui";

/** Restaure une sauvegarde chiffrée (`.vigiebak`) ; le mot de passe est celui de la sauvegarde. */
export function RestoreBackup({ onCancel }: { onCancel: () => void }) {
  const qc = useQueryClient();
  const [pw, setPw] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      const restored = await unwrap(commands.restoreVaultBackup(pw));
      setPw("");
      if (restored) await qc.invalidateQueries({ queryKey: VAULT_KEY });
    } catch (err) {
      setError(errorMessage(err));
      setPw("");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={t("Restaurer une sauvegarde")}>
      <p className="text-sm text-muted">
        {t(
          "Choisis un fichier .vigiebak. Entre le mot de passe maître qui protégeait ce coffre. Si un coffre existe déjà ici, il est conservé à côté, renommé.",
        )}
      </p>
      <form onSubmit={submit} className="space-y-4">
        <Field
          label={t("Mot de passe maître de la sauvegarde")}
          type="password"
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          autoFocus
        />
        <ErrorText>{error}</ErrorText>
        <div className="flex gap-2">
          <Button type="submit" disabled={busy || !pw}>
            {busy ? t("Restauration…") : t("Choisir le fichier et restaurer")}
          </Button>
          <Button type="button" variant="ghost" onClick={onCancel}>
            {t("Annuler")}
          </Button>
        </div>
      </form>
    </Card>
  );
}
