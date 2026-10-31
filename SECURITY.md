# Politique de sécurité

Vigie est en développement (avant la v1.0) ; seule la dernière version reçoit des correctifs.

## Signaler une vulnérabilité

Ouvre un **avis de sécurité privé** (onglet *Security* → *Report a vulnerability* du dépôt GitHub). N'inclus jamais de vraies données personnelles ni d'export de mots de passe. Je réponds sous 7 jours et je te tiens informé jusqu'au correctif ; la divulgation publique se fait après la sortie du correctif (90 jours maximum).

## Modèle de sécurité en bref

- Les données sont chiffrées au repos (SQLCipher, AES-256). La clé de la base est enveloppée par le mot de passe maître (Argon2id) et par une clé de récupération.
- Aucun mot de passe, ni hash de mot de passe, n'est stocké. Les imports sont lus par le cœur Rust et effacés de la mémoire.
- Les appels réseau ne partent que du cœur Rust, en HTTPS, vers une liste blanche de 3 hôtes ; aucune télémétrie.
- La WebView applique une CSP stricte et des capacités Tauri minimales.

Le détail du modèle de sécurité figure dans le [README](README.md#modèle-de-sécurité).

## Hors périmètre

Un logiciel malveillant avec privilèges **root ou noyau** peut lire la mémoire, enregistrer le clavier et contourner toute protection : Vigie ne s'en défend pas. De même, tant que le coffre est déverrouillé, ses données sont accessibles à un processus qui s'exécute sous ton compte. Active FileVault et verrouille ta session.

Autres limites connues : la réutilisation de mots de passe n'est détectée que parmi les comptes d'un même import ; les vérifications Pwned Passwords révèlent un préfixe de hash de 5 caractères ; la vérification par courriel (option) envoie le courriel en clair à HIBP.
