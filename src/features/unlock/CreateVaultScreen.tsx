import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { VAULT_KEY } from "../../lib/useVault";
import { Button, Card, ErrorText, Field } from "../../components/ui";
import { RecoveryKeyScreen } from "./RecoveryKeyScreen";
import { StrengthMeter } from "./StrengthMeter";
import { t } from "../../i18n";

export function CreateVaultScreen() {
  const qc = useQueryClient();
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [recovery, setRecovery] = useState<string>();

  if (recovery) {
    // Le coffre est déjà créé et déverrouillé ; on ne bascule qu'une fois la clé confirmée.
    return (
      <RecoveryKeyScreen
        recoveryKey={recovery}
        onDone={() => {
          setRecovery(undefined);
          void qc.invalidateQueries({ queryKey: VAULT_KEY });
        }}
      />
    );
  }

  const mismatch = confirm.length > 0 && pw !== confirm;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (pw !== confirm) return;
    setBusy(true);
    setError(undefined);
    try {
      const key = await unwrap(commands.createVault(pw));
      setPw("");
      setConfirm("");
      setRecovery(key);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={t("Créer ton coffre")}>
      <p className="text-sm text-zinc-400">
        {t(
          "Ce mot de passe maître chiffre tout l'inventaire. Il ne peut pas être récupéré sans la clé de récupération que tu verras ensuite.",
        )}
      </p>
      <form onSubmit={submit} className="space-y-4">
        <Field
          label={t("Mot de passe maître")}
          type="password"
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          autoFocus
        />
        <StrengthMeter password={pw} />
        <Field
          label={t("Confirmation")}
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
        />
        <ErrorText>{mismatch ? t("Les mots de passe ne correspondent pas.") : error}</ErrorText>
        <Button type="submit" disabled={busy || !pw || pw !== confirm}>
          {busy ? t("Création…") : t("Créer le coffre")}
        </Button>
      </form>
    </Card>
  );
}
