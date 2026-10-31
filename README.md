# Vigie

[![CI](https://github.com/yassineeljal/Vigie/actions/workflows/ci.yml/badge.svg)](https://github.com/yassineeljal/Vigie/actions/workflows/ci.yml)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)
![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon%20%2B%20Intel-lightgrey)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db)
![Rust](https://img.shields.io/badge/c%C5%93ur-Rust-b7410e)

Inventaire de sécurité de tes comptes en ligne, **local et chiffré** (macOS). Vigie recense tes comptes, détecte les risques (MFA absente, mot de passe réutilisé ou fuité, brèches), calcule un score **explicable** et te dit **quoi corriger en premier**.

> Statut : version de développement, avant la v1.0. Voir [CHANGELOG.md](CHANGELOG.md) et le plan complet dans [docs/PLAN.md](docs/PLAN.md).

## Pourquoi Vigie existe

On a des dizaines de comptes et on ne sait pas lesquels n'ont pas de MFA, lesquels réutilisent un mot de passe, lesquels ont subi une brèche, ni quel courriel sert à récupérer tous les autres. Vigie répond à une question : **par où commencer ?**

Vigie n'est **pas** un gestionnaire de mots de passe : il ne stocke aucun mot de passe et ne les remplit pas.

## Fonctionnalités

- Coffre chiffré (SQLCipher, clé protégée par Argon2id), clé de récupération imprimable, verrouillage automatique
- Inventaire : comptes, identités (courriels, téléphones), méthodes MFA, criticité, étiquettes, recherche et filtres
- Import Bitwarden (CSV/JSON), app Mots de passe d'Apple, CSV quelconque ; détection des mots de passe réutilisés et des doublons
- Renseignement : Pwned Passwords (k-anonymat), brèches publiques Have I Been Pwned corrélées par domaine, 2fa.directory, courriel en option
- Score global et par compte avec ses facteurs, plan d'action trié par **impact** ou **quick wins**
- Tableau de bord, historique du score, graphe de dépendances de récupération, notifications (sans noms de comptes)
- Export JSON/CSV sans secrets, sauvegarde chiffrée, interface en français et en anglais, navigation au clavier

## Confidentialité : ce qui sort de ta machine

| Fonction | Ce qui est envoyé | Quand |
|---|---|---|
| Mot de passe fuité ? (Pwned Passwords) | Les **5 premiers caractères** du SHA-1 du mot de passe, avec `Add-Padding` | Seulement quand tu le demandes (vérification ou import) |
| Brèches d'un **service** | **Rien** : téléchargement du catalogue public `/breaches` | Synchronisation quotidienne (désactivable) |
| Méthodes MFA d'un site (2fa.directory) | **Rien** : téléchargement du catalogue public | Avec la synchronisation |
| Brèches d'un **courriel** (option) | Le **courriel en clair**, avec **ta** clé API HIBP (payante) | Seulement après confirmation explicite |

Aucune télémétrie. Aucune synchronisation dans le nuage. Aucun favicon téléchargé. Le **mode hors ligne** coupe tout appel réseau. Les appels réseau ne partent que du cœur Rust, vers 3 hôtes en liste blanche (voir [ADR-006](docs/adr/ADR-006-reseau.md)).

## Installation

1. Télécharge le `.dmg` de la dernière [release](../../releases) et vérifie son empreinte : `shasum -a 256 -c SHA256SUMS.txt`.
2. Glisse **Vigie** dans Applications.
3. Tant que l'app n'est pas notarisée, macOS (Gatekeeper) refuse de l'ouvrir : fais un clic droit sur l'app → **Ouvrir**, ou retire la quarantaine :
   ```sh
   xattr -dr com.apple.quarantine /Applications/Vigie.app
   ```
   Voir [ADR-008](docs/adr/ADR-008-distribution.md) pour le choix de ne pas signer la v1.

## Utilisation en 3 étapes

1. **Créer le coffre** : choisis une longue phrase de passe ; imprime ta clé de récupération.
2. **Importer** : exporte depuis ton gestionnaire, choisis le fichier, vérifie l'aperçu (doublons, réutilisation, fuites), importe, puis **supprime le fichier d'export**.
3. **Suivre le plan d'action** : traite les actions de haut en bas ; chaque action faite met le score à jour.

Raccourcis : `⌘L` verrouille, `Alt+1…8` change d'onglet, `Échap` ferme un panneau.

## Développement

Prérequis : Rust (rustup), Node 22+.

```sh
npm install
npm run tauri dev          # app en développement
cd src-tauri && cargo test # tests Rust (≈ 100)
npm test                   # tests front
```

CI : `cargo fmt`, `clippy -D warnings`, `cargo test`, `cargo deny`, ESLint, `tsc`, Vitest, `npm audit`, gitleaks. Les types IPC TypeScript (`src/lib/ipc.ts`) sont générés par `cargo test`.

## Architecture et modèle de sécurité

- [docs/architecture.md](docs/architecture.md) · [docs/threat-model.md](docs/threat-model.md) · [docs/security-review.md](docs/security-review.md)
- [Décisions d'architecture (ADR)](docs/adr/) · [SECURITY.md](SECURITY.md)

## Roadmap

v1.1 : Touch ID, icône de barre de menu, rapport HTML/PDF, mises à jour signées. v2 : voir [docs/PLAN.md](docs/PLAN.md) §13. Contribuer : [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence et remerciements

[MIT](LICENSE). Données de brèches et mots de passe compromis : [Have I Been Pwned](https://haveibeenpwned.com) (Troy Hunt). Méthodes d'authentification : [2fa.directory](https://2fa.directory). Respecte leurs conditions d'utilisation.
