import { scoreColor } from "../../lib/factors";
import { useScoreReport } from "./queries";
import { t } from "../../i18n";

/** Score global (US-41) : 100 − risque pondéré moyen. */
export function ScoreBadge() {
  const { data } = useScoreReport();
  if (!data) return null;
  const score = Math.round(data.global ?? 100);
  return (
    <p className="text-sm text-zinc-400" aria-label={t("Score global {score} sur 100", { score })}>
      {t("Score")} <span className={`text-lg font-semibold ${scoreColor(score)}`}>{score}</span>
      <span className="text-zinc-500">/100</span>
    </p>
  );
}
