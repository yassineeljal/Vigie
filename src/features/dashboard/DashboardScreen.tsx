import { useQuery } from "@tanstack/react-query";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { commands, type Dashboard } from "../../lib/ipc";
import { unwrap } from "../../lib/result";
import { ACTION_LABELS, FACTOR_LABELS, scoreColor } from "../../lib/factors";
import { CRITICALITY_LABELS } from "../../lib/labels";

const BUCKETS: Record<string, { label: string; color: string }> = {
  low: { label: "Faible", color: "bg-emerald-500" },
  moderate: { label: "Modéré", color: "bg-amber-400" },
  high: { label: "Élevé", color: "bg-orange-500" },
  severe: { label: "Critique", color: "bg-red-500" },
};

export const useDashboard = () =>
  useQuery({ queryKey: ["dashboard"], queryFn: () => unwrap(commands.getDashboard()) });

export function DashboardScreen({
  onNavigate,
}: {
  onNavigate: (tab: "actions" | "breaches") => void;
}) {
  const { data } = useDashboard();
  if (!data) return null;
  const score = Math.round(data.score ?? 100);

  return (
    <section className="max-w-5xl space-y-6 overflow-y-auto pb-8">
      <h2 className="sr-only">Tableau de bord</h2>
      <div className="grid gap-4 md:grid-cols-4">
        <div className="rounded-2xl border border-zinc-800 p-5 md:col-span-1">
          <p className="text-xs text-zinc-400">Score global</p>
          <p
            className={`text-5xl font-semibold ${scoreColor(score)}`}
            aria-label={`Score ${score} sur 100`}
          >
            {score}
          </p>
          <Delta value={data.delta_30d ?? null} />
        </div>
        <Tile label="Comptes" value={data.accounts_total} />
        <Tile label="Sans MFA" value={data.without_mfa} warn={data.without_mfa > 0} />
        <Tile
          label="Critiques sans MFA forte"
          value={data.critical_without_strong_mfa}
          warn={data.critical_without_strong_mfa > 0}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card title="Évolution du score">
          <History points={data.history} />
        </Card>
        <Card title="Répartition du risque">
          <Bars
            rows={data.risk_buckets.map((b) => ({
              label: BUCKETS[b.level].label,
              value: b.count,
              color: BUCKETS[b.level].color,
            }))}
          />
          <h4 className="mt-4 mb-2 text-xs text-zinc-400">Par criticité</h4>
          <Bars
            rows={data.by_criticality.map((c) => ({
              label: CRITICALITY_LABELS[c.criticality],
              value: c.count,
              color: "bg-sky-500",
            }))}
          />
        </Card>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card
          title={`Top ${data.top_actions.length} des actions`}
          action={`${data.pending_actions} en attente · gain potentiel ${Math.round(data.potential_gain ?? 0)}`}
          onAction={() => onNavigate("actions")}
        >
          {data.top_actions.length === 0 ? (
            <p className="text-sm text-emerald-400">Rien à corriger. Bravo !</p>
          ) : (
            <ol className="space-y-2 text-sm">
              {data.top_actions.map((a) => (
                <li key={a.id} className="flex justify-between gap-3">
                  <span className="min-w-0">
                    <span className="font-medium">{a.account_label}</span>
                    <span className="block truncate text-xs text-zinc-400">
                      {ACTION_LABELS[a.kind]}
                    </span>
                  </span>
                  <span className="shrink-0 text-emerald-400">−{(a.gain ?? 0).toFixed(1)}</span>
                </li>
              ))}
            </ol>
          )}
        </Card>

        <Card title="Brèches récentes" action="Voir tout" onAction={() => onNavigate("breaches")}>
          {data.recent_breaches.length === 0 ? (
            <p className="text-sm text-zinc-500">Aucune brèche pertinente à traiter.</p>
          ) : (
            <ul className="space-y-2 text-sm">
              {data.recent_breaches.map((m) => (
                <li key={`${m.breach_name}-${m.account_id}-${m.kind}`}>
                  <span className="font-medium">{m.account_label}</span>{" "}
                  <span className="text-zinc-400">
                    · {m.title} ({m.breach_date?.slice(0, 7) ?? "?"})
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      {data.top_factors.length > 0 && (
        <Card title="Facteurs de risque les plus fréquents">
          <Bars
            rows={data.top_factors.slice(0, 6).map((f) => ({
              label: FACTOR_LABELS[f.factor],
              value: f.accounts,
              color: "bg-rose-500",
            }))}
          />
        </Card>
      )}
    </section>
  );
}

function Tile({ label, value, warn }: { label: string; value: number; warn?: boolean }) {
  return (
    <div className="rounded-2xl border border-zinc-800 p-5">
      <p className="text-xs text-zinc-400">{label}</p>
      <p className={`text-3xl font-semibold ${warn ? "text-amber-400" : ""}`}>{value}</p>
    </div>
  );
}

function Delta({ value }: { value: number | null }) {
  if (value === null) return <p className="text-xs text-zinc-500">Pas encore d'historique</p>;
  const v = Math.round(value * 10) / 10;
  return (
    <p className={`text-xs ${v >= 0 ? "text-emerald-400" : "text-red-400"}`}>
      {v >= 0 ? "▲" : "▼"} {Math.abs(v)} sur 30 jours
    </p>
  );
}

function Card({
  title,
  action,
  onAction,
  children,
}: {
  title: string;
  action?: string;
  onAction?: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-3 rounded-2xl border border-zinc-800 p-5">
      <div className="flex items-baseline justify-between gap-3">
        <h3 className="text-sm font-medium text-zinc-300">{title}</h3>
        {action && (
          <button onClick={onAction} className="text-xs text-sky-300 hover:underline">
            {action}
          </button>
        )}
      </div>
      {children}
    </div>
  );
}

function Bars({ rows }: { rows: { label: string; value: number; color: string }[] }) {
  const max = Math.max(1, ...rows.map((r) => r.value));
  return (
    <ul className="space-y-1.5">
      {rows.map((r) => (
        <li key={r.label} className="grid grid-cols-[9rem_1fr_2rem] items-center gap-2 text-xs">
          <span className="truncate text-zinc-300" title={r.label}>
            {r.label}
          </span>
          <span className="h-2 rounded bg-zinc-800">
            <span
              className={`block h-full rounded ${r.color}`}
              style={{ width: `${(r.value / max) * 100}%` }}
            />
          </span>
          <span className="text-right text-zinc-400">{r.value}</span>
        </li>
      ))}
    </ul>
  );
}

function History({ points }: { points: Dashboard["history"] }) {
  if (points.length < 2) {
    return (
      <p className="text-sm text-zinc-500">
        L'historique se construit jour après jour : reviens demain pour voir la courbe.
      </p>
    );
  }
  const data = points.map((p) => ({
    date: p.taken_at.slice(0, 10),
    score: Math.round((p.score ?? 0) * 10) / 10,
  }));
  return (
    <div role="img" aria-label={`Évolution du score sur ${points.length} jours`} className="h-48">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={data} margin={{ top: 5, right: 10, left: -20, bottom: 0 }}>
          <CartesianGrid stroke="#27272a" strokeDasharray="3 3" />
          <XAxis dataKey="date" stroke="#71717a" fontSize={11} />
          <YAxis domain={[0, 100]} stroke="#71717a" fontSize={11} />
          <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #3f3f46" }} />
          <Line type="monotone" dataKey="score" stroke="#34d399" strokeWidth={2} dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
