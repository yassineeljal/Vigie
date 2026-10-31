import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { VAULT_KEY } from "../../lib/useVault";
import { Button, ErrorText, Field } from "../../components/ui";
import { StrengthMeter } from "../unlock/StrengthMeter";

const DELAYS = [
  { value: 1, label: "1 minute" },
  { value: 5, label: "5 minutes" },
  { value: 15, label: "15 minutes" },
  { value: 60, label: "1 heure" },
  { value: 0, label: "Jamais" },
];

export function SettingsScreen({ autoLockMinutes }: { autoLockMinutes: number }) {
  const qc = useQueryClient();
  const [oldPw, setOldPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string }>();

  const notif = useQuery({
    queryKey: ["notifications-enabled"],
    queryFn: () => unwrap(commands.getNotificationsEnabled()).catch(() => false),
  });
  const [notifError, setNotifError] = useState<string>();

  async function toggleNotifications(enabled: boolean) {
    setNotifError(undefined);
    try {
      await unwrap(commands.setNotificationsEnabled(enabled));
    } catch (e) {
      setNotifError(errorMessage(e));
    }
    await qc.invalidateQueries({ queryKey: ["notifications-enabled"] });
  }

  async function changeDelay(minutes: number) {
    await unwrap(commands.setAutoLockMinutes(minutes));
    await qc.invalidateQueries({ queryKey: VAULT_KEY });
  }

  async function changePassword(e: React.FormEvent) {
    e.preventDefault();
    try {
      await unwrap(commands.changeMasterPassword(oldPw, newPw));
      setMsg({ ok: true, text: "Mot de passe maître modifié." });
      setOldPw("");
      setNewPw("");
    } catch (err) {
      setMsg({ ok: false, text: errorMessage(err) });
      setOldPw("");
    }
  }

  return (
    <section className="space-y-8">
      <h2 className="text-xl font-semibold">Réglages</h2>

      <div className="space-y-2">
        <label htmlFor="auto-lock" className="block text-sm text-zinc-300">
          Verrouillage automatique après inactivité
        </label>
        <select
          id="auto-lock"
          value={autoLockMinutes}
          onChange={(e) => void changeDelay(Number(e.target.value))}
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm"
        >
          {DELAYS.map((d) => (
            <option key={d.value} value={d.value}>
              {d.label}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1">
        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            checked={notif.data ?? false}
            onChange={(e) => void toggleNotifications(e.target.checked)}
          />
          Notifications macOS (nouvelle brèche pertinente, rappel hebdomadaire)
        </label>
        <p className="text-xs text-zinc-500">
          Les notifications n'affichent jamais de noms de comptes. Elles ne partent que lorsque le
          coffre est déverrouillé.
        </p>
        <ErrorText>{notifError}</ErrorText>
      </div>

      <form onSubmit={changePassword} className="max-w-sm space-y-3">
        <h3 className="text-sm font-medium text-zinc-300">Changer le mot de passe maître</h3>
        <Field
          label="Mot de passe actuel"
          type="password"
          value={oldPw}
          onChange={(e) => setOldPw(e.target.value)}
        />
        <Field
          label="Nouveau mot de passe"
          type="password"
          value={newPw}
          onChange={(e) => setNewPw(e.target.value)}
        />
        <StrengthMeter password={newPw} />
        {msg &&
          (msg.ok ? (
            <p className="text-sm text-emerald-400">{msg.text}</p>
          ) : (
            <ErrorText>{msg.text}</ErrorText>
          ))}
        <Button type="submit" disabled={!oldPw || !newPw}>
          Modifier
        </Button>
      </form>
    </section>
  );
}
