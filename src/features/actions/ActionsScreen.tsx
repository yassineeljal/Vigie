import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ActionStatus,
  type ActionView,
  type MfaKind,
  type PlanView,
} from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { ACTION_LABELS, FACTOR_LABELS } from "../../lib/factors";
import { MFA_LABELS } from "../../lib/labels";
import { Button, ErrorText } from "../../components/ui";
import { ScoreBadge } from "../score/ScoreBadge";

const STRONG: MfaKind[] = ["passkey", "totp", "hardware_key"];
const sel = "rounded-lg border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm";
const STATUS_TABS: { value: ActionStatus; label: string }[] = [
  { value: "todo", label: "À faire" },
  { value: "done", label: "Faites" },
  { value: "ignored", label: "Ignorées" },
];

export function ActionsScreen() {
  const qc = useQueryClient();
  const [view, setView] = useState<PlanView>("impact");
  const [status, setStatus] = useState<ActionStatus>("todo");
  const [error, setError] = useState<string>();
  const { data: actions = [] } = useQuery({
    queryKey: ["actions", view, status],
    queryFn: () => unwrap(commands.listActions(view, status)),
  });
  const refresh = () => {
    for (const k of ["actions", "score", "accounts", "dashboard", "graph"])
      void qc.invalidateQueries({ queryKey: [k] });
  };
  const complete = useMutation({
    mutationFn: (v: { id: number; mfa: MfaKind | null }) =>
      unwrap(commands.completeAction(v.id, v.mfa)),
    onSuccess: () => {
      setError(undefined);
      refresh();
    },
    onError: (e) => setError(errorMessage(e)),
  });
  const setStatusMut = useMutation({
    mutationFn: (v: { id: number; status: ActionStatus }) =>
      unwrap(commands.setActionStatus(v.id, v.status)),
    onSuccess: refresh,
  });

  return (
    <section className="max-w-4xl space-y-5 overflow-y-auto pb-8">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-xl font-semibold">Plan d'action</h2>
        <ScoreBadge />
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div role="group" aria-label="Tri" className="flex gap-1">
          <Button
            variant={view === "impact" ? "primary" : "ghost"}
            aria-pressed={view === "impact"}
            onClick={() => setView("impact")}
          >
            Impact
          </Button>
          <Button
            variant={view === "quick_wins" ? "primary" : "ghost"}
            aria-pressed={view === "quick_wins"}
            onClick={() => setView("quick_wins")}
          >
            Quick wins
          </Button>
        </div>
        <div role="group" aria-label="Statut" className="flex gap-1">
          {STATUS_TABS.map((t) => (
            <Button
              key={t.value}
              variant={status === t.value ? "primary" : "ghost"}
              aria-pressed={status === t.value}
              onClick={() => setStatus(t.value)}
            >
              {t.label}
            </Button>
          ))}
        </div>
      </div>
      <ErrorText>{error}</ErrorText>

      {actions.length === 0 ? (
        <p className="text-sm text-zinc-500">
          {status === "todo" ? "Rien à corriger pour le moment. Bravo !" : "Aucune action."}
        </p>
      ) : (
        <ol className="space-y-3">
          {actions.map((a) => (
            <ActionCard
              key={a.id}
              a={a}
              busy={complete.isPending}
              onDone={(mfa) => complete.mutate({ id: a.id, mfa })}
              onStatus={(s) => setStatusMut.mutate({ id: a.id, status: s })}
            />
          ))}
        </ol>
      )}
    </section>
  );
}

function mfaHint(a: ActionView): string | null {
  if (a.kind !== "enable_mfa" && a.kind !== "upgrade_mfa" && a.kind !== "secure_pivot") return null;
  if (a.supports_passkeys) return "Ce site supporte les passkeys : active-les.";
  if (a.supported_mfa.includes("u2f")) return "Ce site supporte les clés de sécurité matérielles.";
  if (a.supported_mfa.includes("totp"))
    return "Ce site supporte le TOTP (application d'authentification).";
  if (a.supported_mfa.length > 0)
    return `Méthodes proposées par le site : ${a.supported_mfa.join(", ")}.`;
  return null;
}

function ActionCard({
  a,
  busy,
  onDone,
  onStatus,
}: {
  a: ActionView;
  busy: boolean;
  onDone: (mfa: MfaKind | null) => void;
  onStatus: (s: ActionStatus) => void;
}) {
  const needsMfa = a.kind === "enable_mfa" || a.kind === "upgrade_mfa" || a.kind === "secure_pivot";
  const defaultMfa: MfaKind = a.supports_passkeys ? "passkey" : "totp";
  const [mfa, setMfa] = useState<MfaKind>(defaultMfa);
  const isPassword = a.kind === "change_password" || a.kind === "change_password_after_breach";
  const hint = mfaHint(a);

  return (
    <li className="space-y-3 rounded-xl border border-zinc-800 p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="font-medium">
            {a.account_label}{" "}
            <span className="text-sm font-normal text-zinc-500">· {a.domain}</span>
          </p>
          <p className="text-sm text-zinc-300">{ACTION_LABELS[a.kind]}</p>
          {hint && <p className="text-xs text-sky-300">{hint}</p>}
          {a.factors.length > 0 && (
            <p className="mt-1 text-xs text-zinc-500">
              Corrige : {a.factors.map((f) => FACTOR_LABELS[f]).join(" · ")}
            </p>
          )}
        </div>
        <dl className="shrink-0 text-right text-xs text-zinc-400">
          <dt className="sr-only">Gain</dt>
          <dd className="text-base font-semibold text-emerald-400">−{(a.gain ?? 0).toFixed(1)}</dd>
          <dt className="sr-only">Effort</dt>
          <dd>{a.effort_min} min</dd>
        </dl>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {a.status === "todo" && (
          <>
            {isPassword && (
              <>
                <Button
                  variant="ghost"
                  onClick={() => void commands.openChangePassword(a.account_id, false)}
                >
                  Ouvrir la page de changement
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => void commands.openChangePassword(a.account_id, true)}
                >
                  Page d'accueil
                </Button>
              </>
            )}
            {needsMfa && (
              <select
                aria-label={`Méthode MFA activée pour ${a.account_label}`}
                className={sel}
                value={mfa}
                onChange={(e) => setMfa(e.target.value as MfaKind)}
              >
                {(a.kind === "enable_mfa" ? (Object.keys(MFA_LABELS) as MfaKind[]) : STRONG).map(
                  (k) => (
                    <option key={k} value={k}>
                      {MFA_LABELS[k]}
                    </option>
                  ),
                )}
              </select>
            )}
            <Button disabled={busy} onClick={() => onDone(needsMfa ? mfa : null)}>
              Marquer comme fait
            </Button>
            <Button variant="ghost" onClick={() => onStatus("ignored")}>
              Ignorer
            </Button>
          </>
        )}
        {a.status !== "todo" && (
          <Button variant="ghost" onClick={() => onStatus("todo")}>
            Rouvrir
          </Button>
        )}
      </div>
    </li>
  );
}
