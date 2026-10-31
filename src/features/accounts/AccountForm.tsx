import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type Account, type AccountInput, type MfaKind } from "../../lib/ipc";
import { errorMessage } from "../../lib/result";
import {
  CRITICALITIES,
  CRITICALITY_LABELS,
  MFA_KINDS,
  MFA_LABELS,
  STATUSES,
  STATUS_LABELS,
} from "../../lib/labels";
import { Button, ErrorText, Field } from "../../components/ui";
import { WhyScore } from "../score/WhyScore";
import { useDeleteAccount, useIdentities, useSaveAccount } from "./queries";
import { t } from "../../i18n";

const EMPTY: AccountInput = {
  domain: "",
  label: "",
  identity_id: null,
  username: null,
  criticality: "medium",
  recovery_identity_id: null,
  pwd_changed_at: null,
  status: "active",
  notes: null,
  tags: [],
  mfa: [],
};

function toInput(a: Account): AccountInput {
  return {
    domain: a.domain,
    label: a.label,
    identity_id: a.identity_id,
    username: a.username,
    criticality: a.criticality,
    recovery_identity_id: a.recovery_identity_id,
    pwd_changed_at: a.pwd_changed_at,
    status: a.status,
    notes: a.notes,
    tags: a.tags,
    mfa: a.mfa,
  };
}

const selectCls = "w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink";

export function AccountForm({
  account,
  onClose,
}: {
  account: Account | null;
  onClose: () => void;
}) {
  const [f, setF] = useState<AccountInput>(account ? toInput(account) : EMPTY);
  const [tags, setTags] = useState(account ? account.tags.join(", ") : "");
  const [error, setError] = useState<string>();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const save = useSaveAccount();
  const del = useDeleteAccount();
  const identities = useIdentities();

  // Échap ferme le panneau (navigation clavier).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const set = <K extends keyof AccountInput>(k: K, v: AccountInput[K]) =>
    setF((p) => ({ ...p, [k]: v }));

  const domainInput = f.domain.trim();
  const { data: preview } = useQuery({
    queryKey: ["preview-domain", domainInput],
    queryFn: () => commands.previewDomain(domainInput),
    enabled: domainInput !== "",
    staleTime: Infinity,
  });

  const hasMfa = (k: MfaKind) => f.mfa.find((m) => m.kind === k);
  function toggleMfa(k: MfaKind) {
    set(
      "mfa",
      hasMfa(k) ? f.mfa.filter((m) => m.kind !== k) : [...f.mfa, { kind: k, backup_codes: false }],
    );
  }
  function toggleBackup(k: MfaKind) {
    set(
      "mfa",
      f.mfa.map((m) => (m.kind === k ? { ...m, backup_codes: !m.backup_codes } : m)),
    );
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(undefined);
    try {
      await save.mutateAsync({
        id: account?.id ?? null,
        input: {
          ...f,
          tags: tags
            .split(",")
            .map((tag) => tag.trim())
            .filter(Boolean),
        },
      });
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const idOptions = (kind?: "email" | "phone") =>
    (identities.data ?? [])
      .filter((i) => !kind || i.kind === kind)
      .map((i) => (
        <option key={i.id} value={i.id}>
          {i.value}
        </option>
      ));
  const numOrNull = (v: string) => (v === "" ? null : Number(v));

  return (
    <div
      className="fixed inset-0 z-10 flex justify-end bg-black/60"
      role="dialog"
      aria-modal="true"
      aria-label={account ? t("Modifier le compte") : t("Nouveau compte")}
    >
      <form
        onSubmit={submit}
        className="h-full w-full max-w-md space-y-4 overflow-y-auto border-l border-line bg-surface p-6"
      >
        <h2 className="text-xl font-semibold">
          {account ? t("Modifier le compte") : t("Nouveau compte")}
        </h2>

        <Field
          label={t("Site (URL ou domaine)")}
          value={f.domain}
          onChange={(e) => set("domain", e.target.value)}
          autoFocus
        />
        {domainInput && (
          <p className="-mt-2 text-xs text-muted" data-testid="domain-preview">
            {preview ? t("Domaine : {preview}", { preview }) : t("Domaine invalide")}
          </p>
        )}
        <Field
          label={t("Libellé")}
          value={f.label}
          onChange={(e) => set("label", e.target.value)}
        />
        <Field
          label={t("Nom d'utilisateur")}
          value={f.username ?? ""}
          onChange={(e) => set("username", e.target.value || null)}
        />

        <label className="block space-y-1 text-sm">
          <span className="text-ink">{t("Identité de connexion")}</span>
          <select
            className={selectCls}
            value={f.identity_id ?? ""}
            onChange={(e) => set("identity_id", numOrNull(e.target.value))}
          >
            <option value="">—</option>
            {idOptions()}
          </select>
        </label>
        <label className="block space-y-1 text-sm">
          <span className="text-ink">{t("Identité de récupération")}</span>
          <select
            className={selectCls}
            value={f.recovery_identity_id ?? ""}
            onChange={(e) => set("recovery_identity_id", numOrNull(e.target.value))}
          >
            <option value="">—</option>
            {idOptions()}
          </select>
        </label>

        <div className="grid grid-cols-2 gap-3">
          <label className="block space-y-1 text-sm">
            <span className="text-ink">{t("Criticité")}</span>
            <select
              className={selectCls}
              value={f.criticality}
              onChange={(e) => set("criticality", e.target.value as AccountInput["criticality"])}
            >
              {CRITICALITIES.map((c) => (
                <option key={c} value={c}>
                  {t(CRITICALITY_LABELS[c])}
                </option>
              ))}
            </select>
          </label>
          <label className="block space-y-1 text-sm">
            <span className="text-ink">{t("Statut")}</span>
            <select
              className={selectCls}
              value={f.status}
              onChange={(e) => set("status", e.target.value as AccountInput["status"])}
            >
              {STATUSES.map((s) => (
                <option key={s} value={s}>
                  {t(STATUS_LABELS[s])}
                </option>
              ))}
            </select>
          </label>
        </div>

        <Field
          label={t("Mot de passe changé le")}
          type="date"
          value={f.pwd_changed_at ?? ""}
          onChange={(e) => set("pwd_changed_at", e.target.value || null)}
        />

        <fieldset className="space-y-2">
          <legend className="text-sm text-ink">{t("Méthodes MFA")}</legend>
          {MFA_KINDS.map((k) => (
            <div key={k} className="flex items-center justify-between text-sm">
              <label className="flex items-center gap-2">
                <input type="checkbox" checked={!!hasMfa(k)} onChange={() => toggleMfa(k)} />
                {t(MFA_LABELS[k])}
              </label>
              {hasMfa(k) && (
                <label className="flex items-center gap-2 text-xs text-muted">
                  <input
                    type="checkbox"
                    checked={hasMfa(k)!.backup_codes}
                    onChange={() => toggleBackup(k)}
                  />
                  {t("codes de secours")}
                </label>
              )}
            </div>
          ))}
        </fieldset>

        <Field
          label={t("Étiquettes (séparées par des virgules)")}
          value={tags}
          onChange={(e) => setTags(e.target.value)}
        />
        <label className="block space-y-1 text-sm">
          <span className="text-ink">{t("Notes (jamais de mot de passe)")}</span>
          <textarea
            className={selectCls}
            rows={3}
            value={f.notes ?? ""}
            onChange={(e) => set("notes", e.target.value || null)}
          />
        </label>

        {account && <WhyScore accountId={account.id} />}
        <ErrorText>{error}</ErrorText>
        <div className="flex items-center justify-between pt-2">
          <div className="flex gap-2">
            <Button type="submit" disabled={save.isPending || !f.domain.trim() || !f.label.trim()}>
              {t("Enregistrer")}
            </Button>
            <Button type="button" variant="ghost" onClick={onClose}>
              {t("Annuler")}
            </Button>
          </div>
          {account &&
            (confirmDelete ? (
              <Button
                type="button"
                onClick={async () => {
                  await del.mutateAsync(account.id);
                  onClose();
                }}
              >
                {t("Confirmer la suppression")}
              </Button>
            ) : (
              <Button type="button" variant="ghost" onClick={() => setConfirmDelete(true)}>
                {t("Supprimer")}
              </Button>
            ))}
        </div>
      </form>
    </div>
  );
}
