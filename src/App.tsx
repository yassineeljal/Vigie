import { useQueryClient } from "@tanstack/react-query";
import { commands } from "./lib/ipc";
import { VAULT_KEY, useVaultLifecycle, useVaultStatus } from "./lib/useVault";
import { Button } from "./components/ui";
import { CreateVaultScreen } from "./features/unlock/CreateVaultScreen";
import { UnlockScreen } from "./features/unlock/UnlockScreen";
import { SettingsScreen } from "./features/settings/SettingsScreen";

export default function App() {
  const qc = useQueryClient();
  const { data: status, isError } = useVaultStatus();
  useVaultLifecycle(status?.unlocked ?? false);

  if (isError) return <p className="p-6 text-red-400">Cœur Rust injoignable.</p>;
  if (!status) return null;
  if (!status.exists) return <CreateVaultScreen />;
  if (!status.unlocked) return <UnlockScreen />;

  return (
    <main className="min-h-screen bg-zinc-950 p-8 text-zinc-100">
      <header className="mb-8 flex items-center justify-between">
        <h1 className="text-3xl font-semibold">Vigie</h1>
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
      <SettingsScreen autoLockMinutes={status.auto_lock_minutes} />
    </main>
  );
}
