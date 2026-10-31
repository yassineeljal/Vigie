import { useQuery } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { t } from "../../i18n";

const LABELS = ["Très faible", "Faible", "Moyen", "Bon", "Excellent"];

export function StrengthMeter({ password }: { password: string }) {
  // La jauge est calculée côté Rust ; le mot de passe n'entre pas dans la clé de requête.
  const { data = 0 } = useQuery({
    queryKey: ["strength", password.length, password.length > 0 ? "set" : "empty"],
    queryFn: () => commands.passwordStrength(password),
    enabled: password.length > 0,
    gcTime: 0,
  });
  if (!password) return null;
  const score = Math.min(data, 3);
  return (
    <div aria-live="polite" className="space-y-1">
      <div className="h-1.5 rounded bg-sunken">
        <div
          className={`h-full rounded ${score >= 3 ? "bg-safe" : "bg-amber-500"}`}
          style={{ width: `${((score + 1) / 4) * 100}%` }}
        />
      </div>
      <p className="text-xs text-muted">
        {score >= 3
          ? t("Suffisant")
          : t("{n1} : choisis une phrase de passe plus longue", { n1: t(LABELS[score]) })}
      </p>
    </div>
  );
}
