import { useQueryClient } from "@tanstack/react-query";
import { commands } from "./lib/ipc";
import { VAULT_KEY, useVaultLifecycle, useVaultStatus } from "./lib/useVault";
import { Suspense, lazy, useEffect, useState } from "react";
import { Button } from "./components/ui";
import { ActionsScreen } from "./features/actions/ActionsScreen";
import { ScoreBadge } from "./features/score/ScoreBadge";
import { AccountsScreen } from "./features/accounts/AccountsScreen";
import { ImportScreen } from "./features/import/ImportScreen";
import { BreachesScreen } from "./features/breaches/BreachesScreen";
import { IdentitiesScreen } from "./features/identities/IdentitiesScreen";
import { RestoreBackup } from "./features/unlock/RestoreBackup";
import { WelcomeScreen } from "./features/unlock/WelcomeScreen";
import { CreateVaultScreen } from "./features/unlock/CreateVaultScreen";
import { UnlockScreen } from "./features/unlock/UnlockScreen";
import { SettingsScreen } from "./features/settings/SettingsScreen";
import { t } from "./i18n";

// Écrans lourds (graphiques, React Flow) chargés à la demande.
const DashboardScreen = lazy(() =>
  import("./features/dashboard/DashboardScreen").then((m) => ({ default: m.DashboardScreen })),
);
const GraphScreen = lazy(() =>
  import("./features/graph/GraphScreen").then((m) => ({ default: m.GraphScreen })),
);

type Tab =
  | "dashboard"
  | "accounts"
  | "actions"
  | "graph"
  | "identities"
  | "import"
  | "breaches"
  | "settings";

const TABS: [Tab, string][] = [
  ["dashboard", "Tableau de bord"],
  ["accounts", "Comptes"],
  ["actions", "Plan d'action"],
  ["graph", "Graphe"],
  ["identities", "Identités"],
  ["import", "Importer"],
  ["breaches", "Brèches"],
  ["settings", "Réglages"],
];

export default function App() {
  const qc = useQueryClient();
  const [tab, setTab] = useState<Tab>("dashboard");
  const [onboarding, setOnboarding] = useState<"welcome" | "create" | "restore">("welcome");
  const { data: status, isError } = useVaultStatus();
  useVaultLifecycle(status?.unlocked ?? false);

  async function lock() {
    await commands.lockVault();
    qc.removeQueries({ predicate: (q) => q.queryKey[0] !== VAULT_KEY[0] });
    await qc.invalidateQueries({ queryKey: VAULT_KEY });
  }

  // Raccourcis clavier : ⌘L / Ctrl+L verrouille, Alt+1…8 change d'onglet.
  useEffect(() => {
    if (!status?.unlocked) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        void lock();
      } else if (e.altKey && /^[1-8]$/.test(e.key)) {
        e.preventDefault();
        setTab(TABS[Number(e.key) - 1][0]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status?.unlocked]);

  if (isError) return <p className="p-6 text-red-400">{t("Cœur Rust injoignable.")}</p>;
  if (!status) return null;
  if (!status.exists) {
    if (onboarding === "restore")
      return <RestoreBackup onCancel={() => setOnboarding("welcome")} />;
    if (onboarding === "welcome") {
      return (
        <WelcomeScreen
          onDone={() => setOnboarding("create")}
          onRestore={() => setOnboarding("restore")}
        />
      );
    }
    return <CreateVaultScreen />;
  }
  if (!status.unlocked) return <UnlockScreen />;

  return (
    <main className="flex h-screen flex-col bg-zinc-950 p-6 text-zinc-100">
      <header className="mb-6 flex items-center justify-between">
        <div className="flex items-baseline gap-4">
          <h1 className="text-2xl font-semibold">{t("Vigie")}</h1>
          <ScoreBadge />
        </div>
        <nav aria-label={t("Navigation")} className="flex flex-wrap gap-1">
          {TABS.map(([k, l]) => (
            <Button
              key={k}
              variant={tab === k ? "primary" : "ghost"}
              aria-current={tab === k ? "page" : undefined}
              onClick={() => setTab(k)}
            >
              {t(l)}
            </Button>
          ))}
        </nav>
        <Button variant="ghost" onClick={() => void lock()}>
          {t("Verrouiller")}
        </Button>
      </header>
      <div className="min-h-0 flex-1">
        <Suspense fallback={<p className="text-sm text-zinc-500">{t("Chargement…")}</p>}>
          {tab === "dashboard" && <DashboardScreen onNavigate={setTab} />}
          {tab === "accounts" && <AccountsScreen />}
          {tab === "graph" && <GraphScreen />}
          {tab === "actions" && <ActionsScreen />}
          {tab === "identities" && <IdentitiesScreen />}
          {tab === "import" && <ImportScreen />}
          {tab === "breaches" && <BreachesScreen />}
          {tab === "settings" && <SettingsScreen autoLockMinutes={status.auto_lock_minutes} />}
        </Suspense>
      </div>
    </main>
  );
}
