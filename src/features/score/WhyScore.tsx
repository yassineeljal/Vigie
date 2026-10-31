import { FACTOR_LABELS } from "../../lib/factors";
import { CRITICALITY_LABELS } from "../../lib/labels";
import { useScoreReport } from "./queries";
import { t } from "../../i18n";

/** Panneau « Pourquoi ce score ? » : chaque facteur et ses points (NFR-10). */
export function WhyScore({ accountId }: { accountId: number }) {
  const { data } = useScoreReport();
  const s = data?.accounts.find((a) => a.account_id === accountId);
  if (!s) return null;
  const sum = s.factors.reduce((n, f) => n + f.points, 0);
  return (
    <section
      aria-label={t("Pourquoi ce score ?")}
      className="space-y-2 rounded-xl border border-line p-3"
    >
      <h3 className="text-sm font-medium text-ink">{t("Pourquoi ce score ?")}</h3>
      {s.factors.length === 0 ? (
        <p className="text-sm text-safe">{t("Aucun facteur de risque.")}</p>
      ) : (
        <ul className="space-y-1 text-sm">
          {s.factors.map((f) => (
            <li key={f.factor} className="flex justify-between gap-3">
              <span>{t(FACTOR_LABELS[f.factor])}</span>
              <span className="text-muted">+{f.points}</span>
            </li>
          ))}
        </ul>
      )}
      <p className="border-t border-line pt-2 text-xs text-muted">
        {t("Probabilité L =")} {s.likelihood}
        {sum > 100 ? t(" (plafonnée à 100)") : ""} {t("× poids")} {s.weight} {t("(criticité")}{" "}
        {t(CRITICALITY_LABELS[s.effective_criticality]).toLowerCase()}
        {t(") = priorité")} <strong className="text-ink">{(s.priority ?? 0).toFixed(1)}</strong>
      </p>
    </section>
  );
}
