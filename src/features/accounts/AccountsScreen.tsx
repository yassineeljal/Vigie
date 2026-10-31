import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Account, AccountFilter, SortKey } from "../../lib/ipc";
import {
  CRITICALITIES,
  CRITICALITY_LABELS,
  STATUSES,
  STATUS_LABELS,
  initials,
} from "../../lib/labels";
import { Badge, Button, Empty, PageHeader } from "../../components/ui";
import { AccountForm } from "./AccountForm";
import { DEFAULT_FILTER, useAccounts, useTags } from "./queries";
import { t } from "../../i18n";

const SORTS: { value: SortKey; label: string }[] = [
  { value: "label", label: "Nom" },
  { value: "domain", label: "Domaine" },
  { value: "criticality", label: "Criticité" },
  { value: "updated_at", label: "Modifié" },
];
const CRIT_DOT: Record<string, string> = {
  critical: "bg-danger",
  high: "bg-warn",
  medium: "bg-warn",
  low: "bg-zinc-500",
};
const sel = "rounded-md border border-line bg-surface px-2.5 py-2 text-sm text-ink";

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

export function AccountsScreen() {
  const [filter, setFilter] = useState<AccountFilter>(DEFAULT_FILTER);
  const [search, setSearch] = useState("");
  const debounced = useDebounced(search, 150);
  const effective = { ...filter, search: debounced || null };
  const { data: accounts = [] } = useAccounts(effective);
  const { data: tags = [] } = useTags();
  const [editing, setEditing] = useState<Account | "new" | null>(null);

  const parentRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual : pas de mémoïsation attendue
  const virt = useVirtualizer({
    count: accounts.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 56,
    overscan: 8,
  });
  const set = <K extends keyof AccountFilter>(k: K, v: AccountFilter[K]) =>
    setFilter((p) => ({ ...p, [k]: v }));

  return (
    <section className="flex h-full flex-col gap-4">
      <PageHeader
        title={t("Comptes")}
        aside={
          <div className="flex items-center gap-3">
            <p className="text-sm text-muted" aria-live="polite">
              {accounts.length} {t("compte(s)")}
            </p>
            <Button onClick={() => setEditing("new")}>{t("Ajouter un compte")}</Button>
          </div>
        }
      />
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="search"
          aria-label={t("Rechercher")}
          placeholder={t("Rechercher un compte…")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className={`${sel} w-64`}
        />
        <select
          aria-label={t("Criticité")}
          className={sel}
          value={filter.criticality ?? ""}
          onChange={(e) =>
            set("criticality", (e.target.value || null) as AccountFilter["criticality"])
          }
        >
          <option value="">{t("Toutes criticités")}</option>
          {CRITICALITIES.map((c) => (
            <option key={c} value={c}>
              {t(CRITICALITY_LABELS[c])}
            </option>
          ))}
        </select>
        <select
          aria-label={t("Statut")}
          className={sel}
          value={filter.status ?? ""}
          onChange={(e) => set("status", (e.target.value || null) as AccountFilter["status"])}
        >
          <option value="">{t("Tous statuts")}</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>
              {t(STATUS_LABELS[s])}
            </option>
          ))}
        </select>
        <select
          aria-label={t("Étiquette")}
          className={sel}
          value={filter.tag ?? ""}
          onChange={(e) => set("tag", e.target.value || null)}
        >
          <option value="">{t("Toutes étiquettes")}</option>
          {tags.map((tag) => (
            <option key={tag} value={tag}>
              {tag}
            </option>
          ))}
        </select>
        <label className="flex items-center gap-1.5 text-sm text-ink">
          <input
            type="checkbox"
            checked={filter.no_mfa}
            onChange={(e) => set("no_mfa", e.target.checked)}
          />
          {t("Sans MFA")}
        </label>
        <select
          aria-label={t("Trier par")}
          className={sel}
          value={filter.sort}
          onChange={(e) => set("sort", e.target.value as SortKey)}
        >
          {SORTS.map((s) => (
            <option key={s.value} value={s.value}>
              {t("Tri :")} {t(s.label)}
            </option>
          ))}
        </select>
        <Button
          variant="ghost"
          aria-label={t("Inverser le tri")}
          onClick={() => set("descending", !filter.descending)}
        >
          {filter.descending ? "↓" : "↑"}
        </Button>
      </div>

      <div
        ref={parentRef}
        className="min-h-0 flex-1 overflow-auto rounded-xl border border-line bg-surface"
        role="list"
        aria-label={t("Comptes")}
      >
        {accounts.length === 0 ? (
          <div className="p-6">
            <Empty>{t("Aucun compte. Ajoutes-en un pour commencer.")}</Empty>
          </div>
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {virt.getVirtualItems().map((row) => {
              const a = accounts[row.index];
              return (
                <button
                  key={a.id}
                  role="listitem"
                  onClick={() => setEditing(a)}
                  className="absolute left-0 flex w-full items-center gap-3 border-b border-line px-4 text-left hover:bg-sunken focus-visible:bg-sunken"
                  style={{ height: row.size, transform: `translateY(${row.start}px)` }}
                >
                  <span
                    aria-hidden
                    className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-sunken font-display text-lg leading-none text-primary"
                  >
                    {initials(a.label)}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{a.label}</span>
                    <span className="block truncate text-xs text-muted">
                      {a.domain}
                      {a.username ? t(" · {username}", { username: a.username }) : ""}
                    </span>
                  </span>
                  <span className="hidden gap-1 sm:flex">
                    {a.tags.slice(0, 3).map((tag) => (
                      <span
                        key={tag}
                        className="rounded-full border border-line px-2 py-0.5 text-xs text-muted"
                      >
                        {tag}
                      </span>
                    ))}
                  </span>
                  <Badge tone={a.mfa.length ? "safe" : "danger"}>
                    {a.mfa.length ? t("MFA ×{length}", { length: a.mfa.length }) : t("Sans MFA")}
                  </Badge>
                  {a.status !== "active" && <Badge tone="warn">{t(STATUS_LABELS[a.status])}</Badge>}
                  <span className="flex items-center gap-1.5 text-xs text-ink">
                    <span className={`h-2 w-2 rounded-full ${CRIT_DOT[a.criticality]}`} />
                    {t(CRITICALITY_LABELS[a.criticality])}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {editing && (
        <AccountForm
          key={editing === "new" ? "new" : editing.id}
          account={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </section>
  );
}
