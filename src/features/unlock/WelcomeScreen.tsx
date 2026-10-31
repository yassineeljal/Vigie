import { useState } from "react";
import { t, useLang, type Lang } from "../../i18n";
import { Button, Card } from "../../components/ui";

const STEPS = [
  {
    title: "Bienvenue dans Vigie",
    body: "Vigie inventorie tes comptes en ligne, repère les risques (MFA absente, mot de passe réutilisé ou fuité, brèches) et te dit quoi corriger en premier. Tout reste sur ton ordinateur, dans un coffre chiffré.",
  },
  {
    title: "Ce que Vigie ne fait pas",
    body: "Vigie n'est pas un gestionnaire de mots de passe : il ne stocke aucun mot de passe et ne les remplit pas. Aucune télémétrie, aucun compte à créer, aucune synchronisation dans le nuage.",
  },
  {
    title: "Ce qui sort de ton ordinateur",
    body: "Seulement, et seulement si tu le demandes : les 5 premiers caractères de l'empreinte d'un mot de passe (Pwned Passwords), le téléchargement de listes publiques (brèches, 2fa.directory) et, en option avec ta propre clé, tes courriels vers Have I Been Pwned.",
  },
  {
    title: "Ton mot de passe maître",
    body: "Il chiffre tout. Choisis une longue phrase de passe que tu retiendras. Tu recevras ensuite une clé de récupération, affichée une seule fois : c'est ta seule porte de secours.",
  },
] as const;

export function WelcomeScreen({
  onDone,
  onRestore,
}: {
  onDone: () => void;
  onRestore: () => void;
}) {
  const [i, setI] = useState(0);
  const [lang, setLang] = useLang();
  const step = STEPS[i];
  const last = i === STEPS.length - 1;
  return (
    <Card title={t(step.title)}>
      <div className="flex justify-end">
        <select
          aria-label="Langue / Language"
          value={lang}
          onChange={(e) => setLang(e.target.value as Lang)}
          className="rounded-md border border-line bg-surface px-2 py-1 text-xs"
        >
          <option value="fr">Français</option>
          <option value="en">English</option>
        </select>
      </div>
      <p className="text-sm leading-relaxed text-ink">{t(step.body)}</p>
      <p className="text-xs text-muted" aria-live="polite">
        {t("Étape {n} sur {total}", { n: i + 1, total: STEPS.length })}
      </p>
      <div className="flex items-center justify-between">
        <div className="flex gap-2">
          {i > 0 && (
            <Button variant="ghost" onClick={() => setI(i - 1)}>
              {t("Retour")}
            </Button>
          )}
          <Button onClick={() => (last ? onDone() : setI(i + 1))} autoFocus>
            {last ? t("Créer mon coffre") : t("Suivant")}
          </Button>
        </div>
        <Button variant="ghost" onClick={onRestore}>
          {t("Restaurer une sauvegarde")}
        </Button>
      </div>
    </Card>
  );
}
