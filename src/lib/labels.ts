import type { AccountStatus, Criticality, IdentityKind, MfaKind } from "./ipc";

export const CRITICALITY_LABELS: Record<Criticality, string> = {
  critical: "Critique",
  high: "Élevée",
  medium: "Moyenne",
  low: "Faible",
};
export const STATUS_LABELS: Record<AccountStatus, string> = {
  active: "Actif",
  unused: "Inutilisé",
  deleted: "À supprimer",
};
export const MFA_LABELS: Record<MfaKind, string> = {
  passkey: "Passkey",
  hardware_key: "Clé matérielle",
  totp: "TOTP",
  push: "Notification push",
  sms: "SMS",
  email: "Courriel",
};
export const IDENTITY_LABELS: Record<IdentityKind, string> = {
  email: "Courriel",
  phone: "Téléphone",
};

export const CRITICALITIES = Object.keys(CRITICALITY_LABELS) as Criticality[];
export const STATUSES = Object.keys(STATUS_LABELS) as AccountStatus[];
export const MFA_KINDS = Object.keys(MFA_LABELS) as MfaKind[];

/** Initiales du service : tient lieu d'icône, aucun favicon n'est téléchargé (aucune fuite réseau). */
export function initials(label: string): string {
  const words = label.trim().split(/\s+/).filter(Boolean);
  const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? "?").slice(0, 2);
  return letters.toUpperCase();
}
