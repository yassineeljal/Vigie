import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, events } from "../../lib/ipc";
import { errorMessage, unwrap } from "../../lib/result";
import { Button, ErrorText, Field } from "../../components/ui";
import { t } from "../../i18n";

const STRENGTH = ["très faible", "faible", "moyen", "bon", "excellent"];

export function useIntelStatus() {
  return useQuery({ queryKey: ["intel-status"], queryFn: () => unwrap(commands.intelStatus()) });
}

function useRefreshOnIntelUpdate() {
  const qc = useQueryClient();
  useEffect(() => {
    const un = events.intelUpdated.listen(() => {
      for (const k of ["breach-matches", "intel-status"])
        void qc.invalidateQueries({ queryKey: [k] });
    });
    return () => void un.then((f) => f());
  }, [qc]);
}

export function BreachesScreen() {
  useRefreshOnIntelUpdate();
  return (
    <section className="max-w-4xl space-y-8 overflow-y-auto pb-8">
      <h2 className="text-xl font-semibold">{t("Brèches")}</h2>
      <CatalogueCard />
      <MatchesList />
      <PasswordChecker />
      <EmailCard />
    </section>
  );
}

function CatalogueCard() {
  const qc = useQueryClient();
  const { data: status } = useIntelStatus();
  const [error, setError] = useState<string>();
  const sync = useMutation({
    mutationFn: () => unwrap(commands.syncBreaches()),
    onSuccess: () => {
      setError(undefined);
      for (const k of ["breach-matches", "intel-status"])
        void qc.invalidateQueries({ queryKey: [k] });
    },
    onError: (e) => setError(errorMessage(e)),
  });
  const offline = useMutation({
    mutationFn: (enabled: boolean) => unwrap(commands.setOfflineMode(enabled)),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["intel-status"] }),
  });
  if (!status) return null;
  return (
    <div className="space-y-3 rounded-xl border border-zinc-800 p-4">
      <p className="text-sm text-zinc-300">
        {status.breaches_synced_at
          ? t("Catalogue public synchronisé le {n1} ({breach_count} brèches).", {
              n1: status.breaches_synced_at.slice(0, 10),
              breach_count: status.breach_count,
            })
          : t("Catalogue public jamais synchronisé.")}{" "}
        <span className="text-zinc-500">
          {t("Le téléchargement n'envoie aucune donnée de ton inventaire.")}
        </span>
      </p>
      <div className="flex flex-wrap items-center gap-4">
        <Button disabled={sync.isPending || status.offline_mode} onClick={() => sync.mutate()}>
          {sync.isPending ? t("Synchronisation…") : t("Synchroniser")}
        </Button>
        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            checked={status.offline_mode}
            onChange={(e) => offline.mutate(e.target.checked)}
          />
          {t("Mode hors ligne (aucun appel réseau)")}
        </label>
      </div>
      {sync.data && (
        <p className="text-sm text-emerald-400">
          {sync.data.breaches} {t("brèches en cache ·")} {sync.data.stats.relevant}{" "}
          {t("pertinente(s) pour tes comptes.")}
        </p>
      )}
      <ErrorText>{error}</ErrorText>
    </div>
  );
}

function MatchesList() {
  const qc = useQueryClient();
  const { data: matches = [] } = useQuery({
    queryKey: ["breach-matches"],
    queryFn: () => unwrap(commands.listBreachMatches()),
  });
  const ack = useMutation({
    mutationFn: (m: { breach_name: string; account_id: number; kind: string }) =>
      unwrap(commands.acknowledgeBreachMatch(m.breach_name, m.account_id, m.kind)),
    onSuccess: () => {
      for (const k of ["breach-matches", "score", "actions", "dashboard"])
        void qc.invalidateQueries({ queryKey: [k] });
    },
  });
  const relevant = matches.filter((m) => m.relevant && !m.acknowledged_at);
  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-zinc-300">
        {t("Comptes concernés (")}
        {relevant.length} {t("à traiter sur")} {matches.length})
      </h3>
      {matches.length === 0 ? (
        <p className="text-sm text-zinc-500">{t("Aucune brèche connue ne touche tes comptes.")}</p>
      ) : (
        <ul className="divide-y divide-zinc-800 rounded-xl border border-zinc-800">
          {matches.map((m) => (
            <li
              key={`${m.breach_name}-${m.account_id}-${m.kind}`}
              className="flex items-center gap-3 p-3 text-sm"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate font-medium">
                  {m.account_label} <span className="text-zinc-500">· {m.title}</span>
                </p>
                <p className="truncate text-xs text-zinc-400">
                  {m.breach_date ?? t("date inconnue")} · {m.data_classes.join(", ")} ·{" "}
                  {m.kind === "email" ? t("via ton courriel") : t("via le domaine")}
                </p>
              </div>
              {m.relevant ? (
                <span className="text-xs text-red-400">
                  {t("Brèche postérieure au dernier changement")}
                </span>
              ) : (
                <span className="text-xs text-zinc-500">{t("Probablement sans impact")}</span>
              )}
              {m.acknowledged_at ? (
                <span className="text-xs text-zinc-500">{t("Acquittée")}</span>
              ) : (
                <Button variant="ghost" onClick={() => ack.mutate(m)}>
                  {t("Acquitter")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function PasswordChecker() {
  const [pw, setPw] = useState("");
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<{ breach_count: number; strength: number }>();
  const check = useMutation({
    mutationFn: (p: string) => unwrap(commands.checkPassword(p, null)),
    onSuccess: (r) => {
      setError(undefined);
      setResult(r);
    },
    onError: (e) => {
      setResult(undefined);
      setError(errorMessage(e));
    },
  });
  return (
    <form
      className="max-w-md space-y-3 rounded-xl border border-zinc-800 p-4"
      onSubmit={(e) => {
        e.preventDefault();
        check.mutate(pw);
        setPw(""); // vidé immédiatement : jamais gardé dans l'état React
      }}
    >
      <h3 className="text-sm font-medium text-zinc-300">{t("Vérifier un mot de passe")}</h3>
      <p className="text-xs text-zinc-500">
        {t(
          "Seuls les 5 premiers caractères de son empreinte SHA-1 sont envoyés (k-anonymat). Le mot de passe n'est ni enregistré ni envoyé.",
        )}
      </p>
      <Field
        label={t("Mot de passe")}
        type="password"
        value={pw}
        onChange={(e) => setPw(e.target.value)}
      />
      <Button type="submit" disabled={!pw || check.isPending}>
        {t("Vérifier")}
      </Button>
      <ErrorText>{error}</ErrorText>
      {result && (
        <p
          role="status"
          className={`text-sm ${result.breach_count > 0 ? "text-red-400" : "text-emerald-400"}`}
        >
          {result.breach_count > 0
            ? t("Trouvé {n1} fois dans des fuites.", {
                n1: result.breach_count.toLocaleString("fr-CA"),
              })
            : t("Absent des fuites connues.")}{" "}
          {t("Force :")} {t(STRENGTH[result.strength])}.
        </p>
      )}
    </form>
  );
}

function EmailCard() {
  const qc = useQueryClient();
  const { data: status } = useIntelStatus();
  const [key, setKey] = useState("");
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string>();
  const done = () => void qc.invalidateQueries({ queryKey: ["intel-status"] });
  const save = useMutation({
    mutationFn: (k: string) => unwrap(commands.setHibpApiKey(k)),
    onSuccess: done,
    onError: (e) => setError(errorMessage(e)),
  });
  const clear = useMutation({
    mutationFn: () => unwrap(commands.clearHibpApiKey()),
    onSuccess: done,
  });
  const run = useMutation({
    mutationFn: () => unwrap(commands.checkIdentityEmails()),
    onSuccess: () => {
      setError(undefined);
      void qc.invalidateQueries({ queryKey: ["breach-matches"] });
    },
    onError: (e) => setError(errorMessage(e)),
  });
  if (!status) return null;
  return (
    <div className="max-w-xl space-y-3 rounded-xl border border-zinc-800 p-4">
      <h3 className="text-sm font-medium text-zinc-300">
        {t("Vérification par courriel (optionnel)")}
      </h3>
      <p className="text-xs text-amber-300">
        {t("Contrairement aux autres vérifications, tes courriels sont envoyés")}{" "}
        <strong>{t("en clair")}</strong>{" "}
        {t(
          "à Have I Been Pwned, avec ta propre clé API (payante). La clé est stockée dans le Trousseau macOS.",
        )}
      </p>
      {status.has_api_key ? (
        <div className="flex flex-wrap items-center gap-3">
          {confirm ? (
            <Button disabled={run.isPending || status.offline_mode} onClick={() => run.mutate()}>
              {run.isPending ? t("Vérification…") : t("Confirmer l'envoi de mes courriels")}
            </Button>
          ) : (
            <Button onClick={() => setConfirm(true)}>{t("Vérifier mes courriels…")}</Button>
          )}
          <Button variant="ghost" onClick={() => clear.mutate()}>
            {t("Supprimer la clé")}
          </Button>
        </div>
      ) : (
        <form
          className="flex items-end gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate(key);
            setKey("");
          }}
        >
          <div className="flex-1">
            <Field
              label={t("Clé API HIBP")}
              type="password"
              value={key}
              onChange={(e) => setKey(e.target.value)}
            />
          </div>
          <Button type="submit" disabled={!key.trim()}>
            {t("Enregistrer")}
          </Button>
        </form>
      )}
      {run.data && (
        <p className="text-sm text-emerald-400">
          {run.data.checked} {t("courriel(s) vérifié(s),")} {run.data.breached} {t("touché(s),")}{" "}
          {run.data.matches} {t("compte(s) concerné(s).")}
        </p>
      )}
      <ErrorText>{error}</ErrorText>
    </div>
  );
}
