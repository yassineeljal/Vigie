import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { VAULT_KEY } from "../../lib/useVault";
import { Button, Card, ErrorText, Field } from "../../components/ui";
import { StrengthMeter } from "./StrengthMeter";
import { t } from "../../i18n";

export function UnlockScreen() {
  const qc = useQueryClient();
  const [mode, setMode] = useState<"password" | "recovery">("password");
  const [pw, setPw] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [newPw, setNewPw] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  function clear() {
    setPw("");
    setRecoveryKey("");
    setNewPw("");
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      if (mode === "password") {
        await unwrap(commands.unlockVault(pw));
      } else {
        await unwrap(commands.resetMasterPassword(recoveryKey, newPw));
        await unwrap(commands.unlockVault(newPw));
      }
      clear();
      await qc.invalidateQueries({ queryKey: VAULT_KEY });
    } catch (err) {
      setError(errorMessage(err));
      setPw("");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={t("Vigie est verrouillé")}>
      <form onSubmit={submit} className="space-y-4">
        {mode === "password" ? (
          <Field
            label={t("Mot de passe maître")}
            type="password"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            autoFocus
          />
        ) : (
          <>
            <Field
              label={t("Clé de récupération")}
              value={recoveryKey}
              onChange={(e) => setRecoveryKey(e.target.value)}
              autoFocus
            />
            <Field
              label={t("Nouveau mot de passe maître")}
              type="password"
              value={newPw}
              onChange={(e) => setNewPw(e.target.value)}
            />
            <StrengthMeter password={newPw} />
          </>
        )}
        <ErrorText>{error}</ErrorText>
        <div className="flex items-center justify-between">
          <Button
            type="submit"
            disabled={busy || (mode === "password" ? !pw : !recoveryKey || !newPw)}
          >
            {busy
              ? t("Déverrouillage…")
              : mode === "password"
                ? t("Déverrouiller")
                : t("Réinitialiser")}
          </Button>
          <Button
            variant="ghost"
            type="button"
            onClick={() => {
              clear();
              setError(undefined);
              setMode(mode === "password" ? "recovery" : "password");
            }}
          >
            {mode === "password" ? t("Mot de passe oublié ?") : t("Retour")}
          </Button>
        </div>
      </form>
    </Card>
  );
}
