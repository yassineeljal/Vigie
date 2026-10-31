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
import { Button } from "../../components/ui";
import { AccountForm } from "./AccountForm";
import { DEFAULT_FILTER, useAccounts, useTags } from "./queries";

const SORTS: { value: SortKey; label: string }[] = [
  { value: "label", label: "Nom" },
  { value: "domain", label: "Domaine" },
  { value: "criticality", label: "Criticité" },
  { value: "updated_at", label: "Modifié" },
];
const CRIT_DOT: Record<string, string> = {
  critical: "bg-red-500",
  high: "bg-orange-500",
  medium: "bg-amber-400",
  low: "bg-zinc-500",
};
const sel = "rounded-lg border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm";

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
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="search"
          aria-label="Rechercher"
          placeholder="Rechercher un compte…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className={`${sel} w-64`}
        />
        <select
          aria-label="Criticité"
          className={sel}
          value={filter.criticality ?? ""}
          onChange={(e) =>
            set("criticality", (e.target.value || null) as AccountFilter["criticality"])
          }
        >
          <option value="">Toutes criticités</option>
          {CRITICALITIES.map((c) => (
            <option key={c} value={c}>
              {CRITICALITY_LABELS[c]}
            </option>
          ))}
        </select>
        <select
          aria-label="Statut"
          className={sel}
          value={filter.status ?? ""}
          onChange={(e) => set("status", (e.target.value || null) as AccountFilter["status"])}
        >
          <option value="">Tous statuts</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>
              {STATUS_LABELS[s]}
            </option>
          ))}
        </select>
        <select
          aria-label="Étiquette"
          className={sel}
          value={filter.tag ?? ""}
          onChange={(e) => set("tag", e.target.value || null)}
        >
          <option value="">Toutes étiquettes</option>
          {tags.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        <label className="flex items-center gap-1.5 text-sm text-zinc-300">
          <input
            type="checkbox"
            checked={filter.no_mfa}
            onChange={(e) => set("no_mfa", e.target.checked)}
          />
          Sans MFA
        </label>
        <select
          aria-label="Trier par"
          className={sel}
          value={filter.sort}
          onChange={(e) => set("sort", e.target.value as SortKey)}
        >
          {SORTS.map((s) => (
            <option key={s.value} value={s.value}>
              Tri : {s.label}
            </option>
          ))}
        </select>
        <Button
          variant="ghost"
          aria-label="Inverser le tri"
          onClick={() => set("descending", !filter.descending)}
        >
          {filter.descending ? "↓" : "↑"}
        </Button>
        <div className="ml-auto">
          <Button onClick={() => setEditing("new")}>Ajouter un compte</Button>
        </div>
      </div>

      <p className="text-xs text-zinc-500" aria-live="polite">
        {accounts.length} compte(s)
      </p>

      <div
        ref={parentRef}
        className="min-h-0 flex-1 overflow-auto rounded-xl border border-zinc-800"
        role="list"
        aria-label="Comptes"
      >
        {accounts.length === 0 ? (
          <p className="p-8 text-center text-sm text-zinc-500">
            Aucun compte. Ajoutes-en un pour commencer.
          </p>
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {virt.getVirtualItems().map((row) => {
              const a = accounts[row.index];
              return (
                <button
                  key={a.id}
                  role="listitem"
                  onClick={() => setEditing(a)}
                  className="absolute left-0 flex w-full items-center gap-3 border-b border-zinc-800/70 px-4 text-left hover:bg-zinc-900 focus-visible:bg-zinc-900"
                  style={{ height: row.size, transform: `translateY(${row.start}px)` }}
                >
                  <span
                    aria-hidden
                    className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-zinc-800 text-xs font-semibold"
                  >
                    {initials(a.label)}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{a.label}</span>
                    <span className="block truncate text-xs text-zinc-400">
                      {a.domain}
                      {a.username ? ` · ${a.username}` : ""}
                    </span>
                  </span>
                  <span className="hidden gap-1 sm:flex">
                    {a.tags.slice(0, 3).map((t) => (
                      <span
                        key={t}
                        className="rounded bg-zinc-800 px-1.5 py-0.5 text-xs text-zinc-300"
                      >
                        {t}
                      </span>
                    ))}
                  </span>
                  <span className={`text-xs ${a.mfa.length ? "text-emerald-400" : "text-red-400"}`}>
                    {a.mfa.length ? `MFA ×${a.mfa.length}` : "Sans MFA"}
                  </span>
                  {a.status !== "active" && (
                    <span className="text-xs text-zinc-400">{STATUS_LABELS[a.status]}</span>
                  )}
                  <span className="flex items-center gap-1.5 text-xs text-zinc-300">
                    <span className={`h-2 w-2 rounded-full ${CRIT_DOT[a.criticality]}`} />
                    {CRITICALITY_LABELS[a.criticality]}
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
