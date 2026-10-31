import { useState } from "react";
import type { IdentityKind } from "../../lib/ipc";
import { errorMessage } from "../../lib/result";
import { IDENTITY_LABELS } from "../../lib/labels";
import { Button, ErrorText, Field } from "../../components/ui";
import {
  useAccounts,
  useDeleteIdentity,
  useIdentities,
  useSaveIdentity,
} from "../accounts/queries";
import { DEFAULT_FILTER } from "../accounts/queries";

export function IdentitiesScreen() {
  const { data: identities = [] } = useIdentities();
  const { data: accounts = [] } = useAccounts(DEFAULT_FILTER);
  const save = useSaveIdentity();
  const del = useDeleteIdentity();
  const [kind, setKind] = useState<IdentityKind>("email");
  const [value, setValue] = useState("");
  const [error, setError] = useState<string>();

  async function add(e: React.FormEvent) {
    e.preventDefault();
    setError(undefined);
    try {
      await save.mutateAsync({ id: null, input: { kind, value, account_id: null } });
      setValue("");
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const sel = "rounded-lg border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm";
  return (
    <section className="max-w-2xl space-y-6">
      <h2 className="text-xl font-semibold">Identités</h2>
      <p className="text-sm text-zinc-400">
        Tes courriels et numéros. Lie un courriel à son compte (ex. ton Gmail) pour repérer les
        comptes dont dépend la récupération des autres.
      </p>
      <form onSubmit={add} className="flex items-end gap-2">
        <select
          aria-label="Type"
          className={sel}
          value={kind}
          onChange={(e) => setKind(e.target.value as IdentityKind)}
        >
          {(Object.keys(IDENTITY_LABELS) as IdentityKind[]).map((k) => (
            <option key={k} value={k}>
              {IDENTITY_LABELS[k]}
            </option>
          ))}
        </select>
        <div className="flex-1">
          <Field label="Valeur" value={value} onChange={(e) => setValue(e.target.value)} />
        </div>
        <Button type="submit" disabled={!value.trim()}>
          Ajouter
        </Button>
      </form>
      <ErrorText>{error}</ErrorText>
      <ul className="divide-y divide-zinc-800 rounded-xl border border-zinc-800">
        {identities.length === 0 && <li className="p-4 text-sm text-zinc-500">Aucune identité.</li>}
        {identities.map((i) => (
          <li key={i.id} className="flex items-center gap-3 p-3 text-sm">
            <span className="w-24 text-xs text-zinc-400">{IDENTITY_LABELS[i.kind]}</span>
            <span className="flex-1 truncate">{i.value}</span>
            <select
              aria-label={`Compte lié à ${i.value}`}
              className={sel}
              value={i.account_id ?? ""}
              onChange={(e) =>
                void save.mutateAsync({
                  id: i.id,
                  input: {
                    kind: i.kind,
                    value: i.value,
                    account_id: e.target.value ? Number(e.target.value) : null,
                  },
                })
              }
            >
              <option value="">Aucun compte lié</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.label}
                </option>
              ))}
            </select>
            <Button
              variant="ghost"
              aria-label={`Supprimer ${i.value}`}
              onClick={() => void del.mutateAsync(i.id)}
            >
              ✕
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
