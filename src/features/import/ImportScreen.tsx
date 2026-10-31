import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ColumnMapping,
  type DuplicatePolicy,
  type FileInspection,
  type ImportFormat,
  type ImportOutcome,
  type ImportPreview,
  type PickedFile,
} from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { Button, ErrorText } from "../../components/ui";

const FORMAT_LABELS: Record<ImportFormat, string> = {
  bitwarden_csv: "Bitwarden (CSV)",
  bitwarden_json: "Bitwarden (JSON)",
  apple_passwords: "Mots de passe d'Apple (CSV)",
  generic_csv: "Autre CSV (mappage manuel)",
};
const STRENGTH = ["très faible", "faible", "moyen", "bon", "excellent"];
const sel = "rounded-lg border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm";

type Step =
  | { name: "pick" }
  | { name: "format"; file: PickedFile; inspection: FileInspection }
  | { name: "preview"; file: PickedFile; preview: ImportPreview }
  | { name: "done"; outcome: ImportOutcome };

export function ImportScreen() {
  const qc = useQueryClient();
  const [step, setStep] = useState<Step>({ name: "pick" });
  const [error, setError] = useState<string>();

  async function guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
    setError(undefined);
    try {
      return await fn();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function cancel() {
    await commands.importCancel();
    setError(undefined);
    setStep({ name: "pick" });
  }

  return (
    <section className="max-w-4xl space-y-5">
      <h2 className="text-xl font-semibold">Importer des comptes</h2>
      <ErrorText>{error}</ErrorText>

      {step.name === "pick" && (
        <PickStep
          onPick={() =>
            guard(async () => {
              const file = await unwrap(commands.importPickFile());
              if (!file) return;
              const inspection = await unwrap(commands.importInspect());
              setStep({ name: "format", file, inspection });
            })
          }
        />
      )}

      {step.name === "format" && (
        <FormatStep
          file={step.file}
          inspection={step.inspection}
          onBack={cancel}
          onStart={(format, mapping) =>
            guard(async () => {
              const preview = await unwrap(commands.importStart(format, mapping));
              setStep({ name: "preview", file: step.file, preview });
            })
          }
        />
      )}

      {step.name === "preview" && (
        <PreviewStep
          preview={step.preview}
          onBack={cancel}
          onCheckLeaks={() =>
            guard(async () => {
              await unwrap(commands.importCheckLeaks());
              const preview = await unwrap(commands.importPreview());
              setStep({ name: "preview", file: step.file, preview });
            })
          }
          onCommit={(policy, excluded) =>
            guard(async () => {
              const outcome = await unwrap(commands.importCommit(policy, excluded));
              for (const k of ["accounts", "tags"]) void qc.invalidateQueries({ queryKey: [k] });
              setStep({ name: "done", outcome });
            })
          }
        />
      )}

      {step.name === "done" && (
        <DoneStep outcome={step.outcome} onAgain={() => setStep({ name: "pick" })} />
      )}
    </section>
  );
}

function PickStep({ onPick }: { onPick: () => void }) {
  return (
    <div className="space-y-4">
      <p className="text-sm text-zinc-400">
        Exporte tes mots de passe depuis Bitwarden (CSV ou JSON non chiffré) ou l'app Mots de passe
        d'Apple, puis choisis le fichier. Il est lu par le cœur de Vigie : les mots de passe ne
        passent jamais par l'interface et ne sont jamais enregistrés. Seuls leur force et leur
        réutilisation sont conservées.
      </p>
      <Button onClick={onPick}>Choisir un fichier…</Button>
    </div>
  );
}

const MAP_FIELDS: { key: keyof ColumnMapping; label: string }[] = [
  { key: "name", label: "Nom du compte" },
  { key: "url", label: "URL / site" },
  { key: "username", label: "Identifiant" },
  { key: "password", label: "Mot de passe" },
  { key: "totp", label: "TOTP (optionnel)" },
  { key: "notes", label: "Notes (optionnel)" },
  { key: "folder", label: "Dossier → étiquette (optionnel)" },
];

function FormatStep({
  file,
  inspection,
  onBack,
  onStart,
}: {
  file: PickedFile;
  inspection: FileInspection;
  onBack: () => void;
  onStart: (f: ImportFormat, m: ColumnMapping | null) => void;
}) {
  const detected = inspection.format;
  const [format, setFormat] = useState<ImportFormat>(detected ?? "generic_csv");
  const [mapping, setMapping] = useState<ColumnMapping>({
    name: null,
    url: null,
    username: null,
    password: null,
    totp: null,
    notes: null,
    folder: null,
  });
  const generic = format === "generic_csv";
  const canStart = !generic || !!(mapping.name || mapping.url);

  return (
    <div className="space-y-4">
      <p className="text-sm text-zinc-300">
        Fichier : <strong>{file.name}</strong> ({Math.ceil(file.size_bytes / 1024)} Kio).{" "}
        {detected
          ? `Format détecté : ${FORMAT_LABELS[detected]}.`
          : "Format non reconnu : indique les colonnes."}
      </p>
      <label className="block space-y-1 text-sm">
        <span className="text-zinc-300">Format</span>
        <select
          className={sel}
          value={format}
          onChange={(e) => setFormat(e.target.value as ImportFormat)}
        >
          {(Object.keys(FORMAT_LABELS) as ImportFormat[]).map((f) => (
            <option key={f} value={f}>
              {FORMAT_LABELS[f]}
            </option>
          ))}
        </select>
      </label>
      {generic && (
        <div className="grid grid-cols-2 gap-3">
          {MAP_FIELDS.map(({ key, label }) => (
            <label key={key} className="block space-y-1 text-sm">
              <span className="text-zinc-300">{label}</span>
              <select
                className={`${sel} w-full`}
                value={mapping[key] ?? ""}
                onChange={(e) => setMapping({ ...mapping, [key]: e.target.value || null })}
              >
                <option value="">—</option>
                {inspection.headers.map((h) => (
                  <option key={h} value={h}>
                    {h}
                  </option>
                ))}
              </select>
            </label>
          ))}
        </div>
      )}
      <div className="flex gap-2">
        <Button disabled={!canStart} onClick={() => onStart(format, generic ? mapping : null)}>
          Analyser
        </Button>
        <Button variant="ghost" onClick={onBack}>
          Annuler
        </Button>
      </div>
    </div>
  );
}

function PreviewStep({
  preview,
  onBack,
  onCheckLeaks,
  onCommit,
}: {
  preview: ImportPreview;
  onBack: () => void;
  onCheckLeaks: () => void;
  onCommit: (p: DuplicatePolicy, excluded: number[]) => void;
}) {
  const [policy, setPolicy] = useState<DuplicatePolicy>("merge");
  const [excluded, setExcluded] = useState<Set<number>>(
    () => new Set(preview.items.filter((i) => i.issue).map((i) => i.index)),
  );
  const commit = useMutation({ mutationFn: async () => onCommit(policy, [...excluded]) });
  const toggle = (i: number) =>
    setExcluded((s) => {
      const n = new Set(s);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });
  const selected = preview.items.length - excluded.size;
  const duplicates = preview.items.filter(
    (i) => i.duplicate_of != null || i.repeats_index != null,
  ).length;
  const checked = preview.items.some((i) => i.breach_count != null);
  const leaked = preview.items.filter((i) => (i.breach_count ?? 0) > 0).length;
  const weak = preview.items.filter((i) => i.strength != null && i.strength <= 1).length;

  return (
    <div className="space-y-4">
      <ul className="flex flex-wrap gap-3 text-sm" aria-label="Résumé">
        <li className="rounded-lg bg-zinc-900 px-3 py-2">{preview.items.length} entrées</li>
        <li className="rounded-lg bg-zinc-900 px-3 py-2">{duplicates} doublon(s)</li>
        <li className="rounded-lg bg-amber-950 px-3 py-2 text-amber-300">
          {preview.reused_accounts} compte(s) avec un mot de passe réutilisé ({preview.reuse_groups}{" "}
          groupe(s))
        </li>
        <li className="rounded-lg bg-zinc-900 px-3 py-2">{weak} mot(s) de passe faible(s)</li>
        {checked && (
          <li
            className={`rounded-lg px-3 py-2 ${leaked ? "bg-red-950 text-red-300" : "bg-zinc-900"}`}
          >
            {leaked} mot(s) de passe dans des fuites
          </li>
        )}
        {preview.skipped_rows > 0 && (
          <li className="rounded-lg bg-zinc-900 px-3 py-2">
            {preview.skipped_rows} ligne(s) ignorée(s)
          </li>
        )}
      </ul>

      <div className="max-h-96 overflow-auto rounded-xl border border-zinc-800">
        <table className="w-full text-left text-sm">
          <thead className="sticky top-0 bg-zinc-900 text-xs text-zinc-400">
            <tr>
              <th className="p-2">
                <span className="sr-only">Importer</span>
              </th>
              <th className="p-2">Compte</th>
              <th className="p-2">Domaine</th>
              <th className="p-2">Notes</th>
            </tr>
          </thead>
          <tbody>
            {preview.items.map((i) => (
              <tr key={i.index} className="border-t border-zinc-800/70">
                <td className="p-2">
                  <input
                    type="checkbox"
                    aria-label={`Importer ${i.label}`}
                    checked={!excluded.has(i.index)}
                    disabled={!!i.issue}
                    onChange={() => toggle(i.index)}
                  />
                </td>
                <td className="p-2">
                  {i.label}
                  {i.username ? <span className="text-zinc-500"> · {i.username}</span> : null}
                </td>
                <td className="p-2 text-zinc-400">{i.domain ?? "—"}</td>
                <td className="space-x-1 p-2 text-xs">
                  {i.issue && <span className="text-red-400">{i.issue}</span>}
                  {(i.duplicate_of != null || i.repeats_index != null) && (
                    <span className="text-sky-300">doublon</span>
                  )}
                  {i.reuse_group != null && (
                    <span className="text-amber-300">réutilisé (groupe {i.reuse_group})</span>
                  )}
                  {i.strength != null && i.strength <= 1 && (
                    <span className="text-red-300">mot de passe {STRENGTH[i.strength]}</span>
                  )}
                  {(i.breach_count ?? 0) > 0 && (
                    <span className="text-red-400">fuité ×{i.breach_count}</span>
                  )}
                  {i.has_totp && <span className="text-emerald-400">TOTP</span>}
                  {!i.has_password && !i.issue && (
                    <span className="text-zinc-500">sans mot de passe</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <label className="flex items-center gap-2 text-sm">
        <span className="text-zinc-300">Doublons :</span>
        <select
          className={sel}
          value={policy}
          onChange={(e) => setPolicy(e.target.value as DuplicatePolicy)}
        >
          <option value="merge">Fusionner avec le compte existant</option>
          <option value="skip">Ignorer</option>
          <option value="keep_both">Créer quand même</option>
        </select>
      </label>

      <div className="flex flex-wrap items-center gap-2">
        <Button variant="ghost" onClick={onCheckLeaks} disabled={checked}>
          {checked ? "Fuites vérifiées" : "Vérifier les fuites (envoie des préfixes de hash)"}
        </Button>
        <Button disabled={commit.isPending || selected === 0} onClick={() => commit.mutate()}>
          Importer {selected} compte(s)
        </Button>
        <Button variant="ghost" onClick={onBack}>
          Annuler
        </Button>
      </div>
    </div>
  );
}

function DoneStep({ outcome, onAgain }: { outcome: ImportOutcome; onAgain: () => void }) {
  const r = outcome.report;
  return (
    <div className="space-y-4">
      <p className="text-sm text-emerald-400">
        Import terminé : {r.created} créé(s), {r.merged} fusionné(s), {r.skipped_duplicates}{" "}
        doublon(s) ignoré(s)
        {r.skipped_invalid > 0 ? `, ${r.skipped_invalid} invalide(s)` : ""}.
      </p>
      <div
        role="alert"
        className="rounded-xl border border-amber-700 bg-amber-950 p-4 text-sm text-amber-200"
      >
        <strong>Supprime maintenant ton fichier d'export</strong>, puis vide la corbeille : il
        contient tes mots de passe en clair.
        <code className="mt-2 block break-all text-xs text-amber-300">{outcome.file_path}</code>
      </div>
      <Button variant="ghost" onClick={onAgain}>
        Importer un autre fichier
      </Button>
    </div>
  );
}
