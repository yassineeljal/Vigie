import { useState } from "react";
import { Button, Card } from "../../components/ui";
import { t } from "../../i18n";

/** Affiche la clé de récupération une seule fois (imprimable, jamais stockée côté front). */
export function RecoveryKeyScreen({
  recoveryKey,
  onDone,
}: {
  recoveryKey: string;
  onDone: () => void;
}) {
  const [saved, setSaved] = useState(false);
  return (
    <Card title={t("Ta clé de récupération")}>
      <p className="text-sm text-muted print:hidden">
        {t(
          "C'est le seul moyen de rouvrir le coffre si tu oublies ton mot de passe maître. Elle ne sera plus jamais affichée. Imprime-la ou note-la, et garde-la hors de cet ordinateur.",
        )}
      </p>
      <p
        data-testid="recovery-key"
        className="rounded-lg border border-line bg-bg p-4 text-center font-mono text-lg leading-relaxed tracking-wider break-words select-all print:border-black print:text-black"
      >
        {recoveryKey}
      </p>
      <div className="print:hidden space-y-4">
        <Button variant="ghost" type="button" onClick={() => window.print()}>
          {t("Imprimer")}
        </Button>
        <label className="flex items-start gap-2 text-sm text-ink">
          <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
          {t("J'ai conservé cette clé en lieu sûr.")}
        </label>
        <Button type="button" disabled={!saved} onClick={onDone}>
          {t("Continuer")}
        </Button>
      </div>
    </Card>
  );
}
