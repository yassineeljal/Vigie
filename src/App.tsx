import { useQueryClient } from "@tanstack/react-query";
import { commands } from "./lib/ipc";
import { VAULT_KEY, useVaultLifecycle, useVaultStatus } from "./lib/useVault";
import { Suspense, lazy, useEffect, useState } from "react";
import {
  AtSign,
  FileInput,
  LayoutDashboard,
  ListChecks,
  Lock,
  Network,
  Settings,
  ShieldAlert,
  Users,
  type LucideIcon,
} from "lucide-react";
import { Wordmark } from "./components/ui";
import { ActionsScreen } from "./features/actions/ActionsScreen";
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

const TABS: [Tab, string, LucideIcon][] = [
  ["dashboard", "Tableau de bord", LayoutDashboard],
  ["accounts", "Comptes", Users],
  ["actions", "Plan d'action", ListChecks],
  ["graph", "Graphe", Network],
  ["identities", "Identités", AtSign],
  ["import", "Importer", FileInput],
  ["breaches", "Brèches", ShieldAlert],
  ["settings", "Réglages", Settings],
];

/** Indices de TABS qui ouvrent un nouveau groupe dans la barre latérale (séparateur). */
const GROUP_STARTS = new Set([1, 5, 7]);

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

  if (isError) return <p className="p-6 text-danger">{t("Cœur Rust injoignable.")}</p>;
  if (!status) return <p className="min-h-screen bg-bg p-6 text-sm text-muted">…</p>;
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
    <div className="flex h-screen bg-bg text-ink">
      <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-surface px-3 py-5">
        <div className="px-2 pb-6">
          <Wordmark />
        </div>
        <nav aria-label={t("Navigation")} className="flex flex-1 flex-col gap-0.5">
          {TABS.map(([k, label, Icon], i) => (
            <div
              key={k}
              className={GROUP_STARTS.has(i) ? "mt-3 border-t border-line pt-3" : undefined}
            >
              <button
                aria-current={tab === k ? "page" : undefined}
                onClick={() => setTab(k)}
                className={`flex w-full items-center gap-3 rounded-md px-2.5 py-2 text-left text-sm transition-colors ${
                  tab === k
                    ? "bg-sunken font-medium text-ink"
                    : "text-muted hover:bg-sunken hover:text-ink"
                }`}
              >
                <Icon
                  size={18}
                  aria-hidden="true"
                  className={tab === k ? "text-primary" : undefined}
                />
                {t(label)}
              </button>
            </div>
          ))}
        </nav>
        <button
          onClick={() => void lock()}
          className="mt-4 flex items-center gap-3 rounded-md border border-line px-2.5 py-2 text-sm text-ink transition-colors hover:bg-sunken"
        >
          <Lock size={18} aria-hidden="true" />
          {t("Verrouiller")}
          <kbd className="ml-auto text-xs text-muted">⌘L</kbd>
        </button>
      </aside>
      <main className="min-w-0 flex-1 overflow-hidden">
        <div className="h-full overflow-y-auto px-10 py-8">
          <Suspense fallback={<p className="text-sm text-muted">{t("Chargement…")}</p>}>
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
    </div>
  );
}
