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
import { ACTION_LABELS, FACTOR_LABELS } from "../../lib/factors";
import { CRITICALITY_LABELS } from "../../lib/labels";
import { t } from "../../i18n";
import { Badge, Empty, PageHeader, Panel } from "../../components/ui";
import { ScoreGauge } from "./ScoreGauge";

const BUCKETS: Record<string, { label: string; color: string }> = {
  low: { label: "Faible", color: "bg-safe" },
  moderate: { label: "Modéré", color: "bg-warn" },
  high: { label: "Élevé", color: "bg-warn" },
  severe: { label: "Critique", color: "bg-danger" },
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
  const score = data.score ?? 100;

  return (
    <section className="max-w-5xl space-y-8 pb-10">
      <PageHeader title={t("Tableau de bord")} />

      <div className="grid items-center gap-8 md:grid-cols-[300px_1fr]">
        <div>
          <ScoreGauge score={score} />
          <div className="mt-2 text-center">
            <Delta value={data.delta_30d ?? null} />
          </div>
        </div>
        <dl className="grid grid-cols-3 divide-x divide-line">
          <Stat label={t("Comptes")} value={data.accounts_total} />
          <Stat label={t("Sans MFA")} value={data.without_mfa} warn={data.without_mfa > 0} />
          <Stat
            label={t("Critiques sans MFA forte")}
            value={data.critical_without_strong_mfa}
            warn={data.critical_without_strong_mfa > 0}
          />
        </dl>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Panel
          title={t("Top {length} des actions", { length: data.top_actions.length })}
          action={
            <button
              onClick={() => onNavigate("actions")}
              className="text-xs text-link hover:underline"
            >
              {t("{pending_actions} en attente · gain potentiel {n2}", {
                pending_actions: data.pending_actions,
                n2: Math.round(data.potential_gain ?? 0),
              })}
            </button>
          }
        >
          {data.top_actions.length === 0 ? (
            <p className="text-sm text-safe">{t("Rien à corriger. Bravo !")}</p>
          ) : (
            <ol className="divide-y divide-line">
              {data.top_actions.map((a) => (
                <li key={a.id} className="flex items-center justify-between gap-3 py-2.5 text-sm">
                  <span className="min-w-0">
                    <span className="block font-medium">{a.account_label}</span>
                    <span className="block truncate text-xs text-muted">
                      {t(ACTION_LABELS[a.kind])}
                    </span>
                  </span>
                  <Badge tone="safe">−{(a.gain ?? 0).toFixed(1)}</Badge>
                </li>
              ))}
            </ol>
          )}
        </Panel>

        <Panel
          title={t("Brèches récentes")}
          action={
            <button
              onClick={() => onNavigate("breaches")}
              className="text-xs text-link hover:underline"
            >
              {t("Voir tout")}
            </button>
          }
        >
          {data.recent_breaches.length === 0 ? (
            <p className="text-sm text-muted">{t("Aucune brèche pertinente à traiter.")}</p>
          ) : (
            <ul className="divide-y divide-line">
              {data.recent_breaches.map((m) => (
                <li key={`${m.breach_name}-${m.account_id}-${m.kind}`} className="py-2.5 text-sm">
                  <span className="font-medium">{m.account_label}</span>{" "}
                  <span className="text-muted">
                    · {m.title} ({m.breach_date?.slice(0, 7) ?? "?"})
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Panel title={t("Évolution du score")}>
          <History points={data.history} />
        </Panel>
        <Panel title={t("Répartition du risque")}>
          <Bars
            rows={data.risk_buckets.map((b) => ({
              label: t(BUCKETS[b.level].label),
              value: b.count,
              color: BUCKETS[b.level].color,
            }))}
          />
          <h4 className="pt-3 text-xs font-medium text-muted">{t("Par criticité")}</h4>
          <Bars
            rows={data.by_criticality.map((c) => ({
              label: t(CRITICALITY_LABELS[c.criticality]),
              value: c.count,
              color: "bg-primary",
            }))}
          />
        </Panel>
      </div>

      {data.top_factors.length > 0 && (
        <Panel title={t("Facteurs de risque les plus fréquents")}>
          <Bars
            rows={data.top_factors.slice(0, 6).map((f) => ({
              label: t(FACTOR_LABELS[f.factor]),
              value: f.accounts,
              color: "bg-danger",
            }))}
          />
        </Panel>
      )}
    </section>
  );
}

function Stat({ label, value, warn }: { label: string; value: number; warn?: boolean }) {
  return (
    <div className="px-5 first:pl-0">
      <dd className={`font-display text-5xl leading-none ${warn ? "text-warn" : "text-ink"}`}>
        {value}
      </dd>
      <dt className="mt-2 text-xs text-muted">{label}</dt>
    </div>
  );
}

function Delta({ value }: { value: number | null }) {
  if (value === null) return <p className="text-xs text-muted">{t("Pas encore d'historique")}</p>;
  const v = Math.round(value * 10) / 10;
  return (
    <p className={`text-sm ${v >= 0 ? "text-safe" : "text-danger"}`}>
      {v >= 0 ? "▲" : "▼"} {Math.abs(v)} {t("sur 30 jours")}
    </p>
  );
}

function Bars({ rows }: { rows: { label: string; value: number; color: string }[] }) {
  const max = Math.max(1, ...rows.map((r) => r.value));
  return (
    <ul className="space-y-2">
      {rows.map((r) => (
        <li key={r.label} className="grid grid-cols-[10rem_1fr_2rem] items-center gap-3 text-xs">
          <span className="truncate text-ink" title={r.label}>
            {r.label}
          </span>
          <span className="h-1.5 rounded-full bg-sunken">
            <span
              className={`block h-full rounded-full ${r.color}`}
              style={{ width: `${(r.value / max) * 100}%` }}
            />
          </span>
          <span className="text-right text-muted">{r.value}</span>
        </li>
      ))}
    </ul>
  );
}

function History({ points }: { points: Dashboard["history"] }) {
  if (points.length < 2) {
    return (
      <Empty>
        {t("L'historique se construit jour après jour : reviens demain pour voir la courbe.")}
      </Empty>
    );
  }
  const data = points.map((p) => ({
    date: p.taken_at.slice(0, 10),
    score: Math.round((p.score ?? 0) * 10) / 10,
  }));
  return (
    <div
      role="img"
      aria-label={t("Évolution du score sur {length} jours", { length: points.length })}
      className="h-48"
    >
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={data} margin={{ top: 5, right: 10, left: -20, bottom: 0 }}>
          <CartesianGrid stroke="var(--line)" strokeDasharray="3 3" />
          <XAxis dataKey="date" stroke="var(--muted)" fontSize={11} />
          <YAxis domain={[0, 100]} stroke="var(--muted)" fontSize={11} />
          <Tooltip
            contentStyle={{
              background: "var(--surface)",
              border: "1px solid var(--line)",
              color: "var(--ink)",
            }}
          />
          <Line
            type="monotone"
            dataKey="score"
            stroke="var(--primary)"
            strokeWidth={2}
            dot={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
