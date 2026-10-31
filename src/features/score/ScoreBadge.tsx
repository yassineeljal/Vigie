import { scoreColor } from "../../lib/factors";
import { useScoreReport } from "./queries";

/** Score global (US-41) : 100 − risque pondéré moyen. */
export function ScoreBadge() {
  const { data } = useScoreReport();
  if (!data) return null;
  const score = Math.round(data.global ?? 100);
  return (
    <p className="text-sm text-zinc-400" aria-label={`Score global ${score} sur 100`}>
      Score <span className={`text-lg font-semibold ${scoreColor(score)}`}>{score}</span>
      <span className="text-zinc-500">/100</span>
    </p>
  );
}
