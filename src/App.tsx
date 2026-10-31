import { useQueryClient } from "@tanstack/react-query";
import { commands } from "./lib/ipc";
import { VAULT_KEY, useVaultLifecycle, useVaultStatus } from "./lib/useVault";
import { useState } from "react";
import { Button } from "./components/ui";
import { ActionsScreen } from "./features/actions/ActionsScreen";
import { ScoreBadge } from "./features/score/ScoreBadge";
import { AccountsScreen } from "./features/accounts/AccountsScreen";
import { ImportScreen } from "./features/import/ImportScreen";
import { BreachesScreen } from "./features/breaches/BreachesScreen";
import { IdentitiesScreen } from "./features/identities/IdentitiesScreen";
import { CreateVaultScreen } from "./features/unlock/CreateVaultScreen";
import { UnlockScreen } from "./features/unlock/UnlockScreen";
import { SettingsScreen } from "./features/settings/SettingsScreen";

export default function App() {
  const qc = useQueryClient();
  const [tab, setTab] = useState<
    "accounts" | "actions" | "identities" | "import" | "breaches" | "settings"
  >("accounts");
  const { data: status, isError } = useVaultStatus();
  useVaultLifecycle(status?.unlocked ?? false);

  if (isError) return <p className="p-6 text-red-400">Cœur Rust injoignable.</p>;
  if (!status) return null;
  if (!status.exists) return <CreateVaultScreen />;
  if (!status.unlocked) return <UnlockScreen />;

  return (
    <main className="flex h-screen flex-col bg-zinc-950 p-6 text-zinc-100">
      <header className="mb-6 flex items-center justify-between">
        <div className="flex items-baseline gap-4">
          <h1 className="text-2xl font-semibold">Vigie</h1>
          <ScoreBadge />
        </div>
        <nav aria-label="Navigation" className="flex gap-1">
          {(
            [
              ["accounts", "Comptes"],
              ["identities", "Identités"],
              ["import", "Importer"],
              ["breaches", "Brèches"],
              ["settings", "Réglages"],
            ] as const
          ).map(([k, l]) => (
            <Button
              key={k}
              variant={tab === k ? "primary" : "ghost"}
              aria-current={tab === k ? "page" : undefined}
              onClick={() => setTab(k)}
            >
              {l}
            </Button>
          ))}
        </nav>
        <Button
          variant="ghost"
          onClick={async () => {
            await commands.lockVault();
            qc.removeQueries({ predicate: (q) => q.queryKey[0] !== VAULT_KEY[0] });
            await qc.invalidateQueries({ queryKey: VAULT_KEY });
          }}
        >
          Verrouiller
        </Button>
      </header>
      <div className="min-h-0 flex-1">
        {tab === "accounts" && <AccountsScreen />}
        {tab === "actions" && <ActionsScreen />}
        {tab === "identities" && <IdentitiesScreen />}
        {tab === "import" && <ImportScreen />}
        {tab === "breaches" && <BreachesScreen />}
        {tab === "settings" && <SettingsScreen autoLockMinutes={status.auto_lock_minutes} />}
      </div>
    </main>
  );
}
