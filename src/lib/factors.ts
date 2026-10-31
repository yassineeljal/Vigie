import type { ActionKind, Factor } from "./ipc";

export const FACTOR_LABELS: Record<Factor, string> = {
  pwned_password: "Mot de passe présent dans des fuites",
  breach_after_change: "Brèche postérieure au dernier changement du mot de passe",
  reused_password: "Mot de passe réutilisé",
  no_mfa: "Aucune authentification multifacteur",
  pivot_without_strong_mfa: "Compte pivot sans MFA forte",
  weak_password: "Mot de passe faible",
  weak_mfa_only: "MFA par SMS ou courriel seulement",
  unused_active: "Compte inutilisé encore actif",
  never_checked: "Mot de passe jamais vérifié",
};

export const ACTION_LABELS: Record<ActionKind, string> = {
  change_password: "Générer un mot de passe unique dans ton gestionnaire et le changer",
  change_password_after_breach: "Changer le mot de passe et vérifier l'activité récente",
  enable_mfa: "Activer une passkey ou du TOTP et sauvegarder les codes de secours",
  upgrade_mfa: "Migrer de SMS/courriel vers TOTP ou une passkey",
  secure_pivot: "Protéger ce compte pivot : MFA forte et codes de secours",
  delete_unused: "Supprimer le compte inutilisé (réduit la surface d'attaque)",
};

export function scoreColor(score: number): string {
  if (score >= 80) return "text-emerald-400";
  if (score >= 50) return "text-amber-400";
  return "text-red-400";
}
