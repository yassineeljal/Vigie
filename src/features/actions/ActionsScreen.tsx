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
import { Button, Empty, ErrorText, PageHeader } from "../../components/ui";
import { ScoreBadge } from "../score/ScoreBadge";
import { t } from "../../i18n";

const STRONG: MfaKind[] = ["passkey", "totp", "hardware_key"];
const sel = "rounded-md border border-line bg-surface px-2.5 py-2 text-sm text-ink";
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
    <section className="max-w-4xl space-y-6 pb-10">
      <PageHeader title={t("Plan d'action")} aside={<ScoreBadge />} />

      <div className="flex flex-wrap items-center gap-3">
        <div role="group" aria-label={t("Tri")} className="flex gap-1">
          <Button
            variant={view === "impact" ? "primary" : "ghost"}
            aria-pressed={view === "impact"}
            onClick={() => setView("impact")}
          >
            {t("Impact")}
          </Button>
          <Button
            variant={view === "quick_wins" ? "primary" : "ghost"}
            aria-pressed={view === "quick_wins"}
            onClick={() => setView("quick_wins")}
          >
            {t("Quick wins")}
          </Button>
        </div>
        <div role="group" aria-label={t("Statut")} className="flex gap-1">
          {STATUS_TABS.map((tab) => (
            <Button
              key={tab.value}
              variant={status === tab.value ? "primary" : "ghost"}
              aria-pressed={status === tab.value}
              onClick={() => setStatus(tab.value)}
            >
              {t(tab.label)}
            </Button>
          ))}
        </div>
      </div>
      <ErrorText>{error}</ErrorText>

      {actions.length === 0 ? (
        <Empty>
          {status === "todo" ? t("Rien à corriger pour le moment. Bravo !") : t("Aucune action.")}
        </Empty>
      ) : (
        <ol>
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
  if (a.supports_passkeys) return t("Ce site supporte les passkeys : active-les.");
  if (a.supported_mfa.includes("u2f"))
    return t("Ce site supporte les clés de sécurité matérielles.");
  if (a.supported_mfa.includes("totp"))
    return t("Ce site supporte le TOTP (application d'authentification).");
  if (a.supported_mfa.length > 0)
    return t("Méthodes proposées par le site : {methods}.", {
      methods: a.supported_mfa.join(", "),
    });
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
    <li className="space-y-3 border-b border-line py-5 last:border-b-0">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="font-medium">
            {a.account_label} <span className="text-sm font-normal text-muted">· {a.domain}</span>
          </p>
          <p className="text-sm text-ink">{t(ACTION_LABELS[a.kind])}</p>
          {hint && <p className="text-xs text-link">{hint}</p>}
          {a.factors.length > 0 && (
            <p className="mt-1 text-xs text-muted">
              {t("Corrige :")} {a.factors.map((f) => t(FACTOR_LABELS[f])).join(" · ")}
            </p>
          )}
        </div>
        <dl className="shrink-0 text-right text-xs text-muted">
          <dt className="sr-only">{t("Gain")}</dt>
          <dd className="font-display text-4xl leading-none text-safe">
            −{(a.gain ?? 0).toFixed(1)}
          </dd>
          <dt className="sr-only">{t("Effort")}</dt>
          <dd>
            {a.effort_min} {t("min")}
          </dd>
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
                  {t("Ouvrir la page de changement")}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => void commands.openChangePassword(a.account_id, true)}
                >
                  {t("Page d'accueil")}
                </Button>
              </>
            )}
            {needsMfa && (
              <select
                aria-label={t("Méthode MFA activée pour {account_label}", {
                  account_label: a.account_label,
                })}
                className={sel}
                value={mfa}
                onChange={(e) => setMfa(e.target.value as MfaKind)}
              >
                {(a.kind === "enable_mfa" ? (Object.keys(MFA_LABELS) as MfaKind[]) : STRONG).map(
                  (k) => (
                    <option key={k} value={k}>
                      {t(MFA_LABELS[k])}
                    </option>
                  ),
                )}
              </select>
            )}
            <Button disabled={busy} onClick={() => onDone(needsMfa ? mfa : null)}>
              {t("Marquer comme fait")}
            </Button>
            <Button variant="ghost" onClick={() => onStatus("ignored")}>
              {t("Ignorer")}
            </Button>
          </>
        )}
        {a.status !== "todo" && (
          <Button variant="ghost" onClick={() => onStatus("todo")}>
            {t("Rouvrir")}
          </Button>
        )}
      </div>
    </li>
  );
}
