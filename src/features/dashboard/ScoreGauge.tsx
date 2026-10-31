import { t } from "../../i18n";

const CX = 130;
const CY = 124;
const R = 104;

/** Point sur le demi-cercle : 0 % à gauche, 100 % à droite. */
function at(fraction: number, radius: number) {
  const a = Math.PI * (1 - fraction);
  return { x: CX + radius * Math.cos(a), y: CY - radius * Math.sin(a) };
}

function arc(from: number, to: number, radius = R) {
  const a = at(from, radius);
  const b = at(to, radius);
  return `M ${a.x.toFixed(2)} ${a.y.toFixed(2)} A ${radius} ${radius} 0 0 1 ${b.x.toFixed(2)} ${b.y.toFixed(2)}`;
}

export function band(score: number): "safe" | "warn" | "danger" {
  if (score >= 80) return "safe";
  if (score >= 50) return "warn";
  return "danger";
}

const STROKE = { safe: "stroke-safe", warn: "stroke-warn", danger: "stroke-danger" } as const;
const TEXT = { safe: "text-safe", warn: "text-warn", danger: "text-danger" } as const;

/** Baromètre de bord : le score sur un demi-cercle gradué (signature visuelle de Vigie). */
export function ScoreGauge({ score }: { score: number }) {
  const value = Math.max(0, Math.min(100, score));
  const b = band(value);
  return (
    <div
      role="img"
      aria-label={t("Score {score} sur 100", { score: Math.round(value) })}
      className="relative mx-auto w-full max-w-[280px]"
    >
      <svg viewBox="0 0 260 150" className="w-full" aria-hidden="true">
        <path
          d={arc(0, 1)}
          fill="none"
          className="stroke-line"
          strokeWidth="12"
          strokeLinecap="round"
        />
        {value > 0 && (
          <path
            d={arc(0, value / 100)}
            fill="none"
            className={STROKE[b]}
            strokeWidth="12"
            strokeLinecap="round"
          />
        )}
        {Array.from({ length: 11 }, (_, i) => {
          const p = at(i / 10, R + 14);
          const q = at(i / 10, R + (i % 5 === 0 ? 24 : 20));
          return (
            <line
              key={i}
              x1={p.x}
              y1={p.y}
              x2={q.x}
              y2={q.y}
              className="stroke-muted"
              strokeWidth={i % 5 === 0 ? 1.6 : 1}
            />
          );
        })}
      </svg>
      <div className="absolute inset-x-0 bottom-1 text-center">
        <p className={`font-display text-7xl leading-none ${TEXT[b]}`}>{Math.round(value)}</p>
        <p className="mt-1 text-xs text-muted">{t("Score global")}</p>
      </div>
    </div>
  );
}
