# Contribuer

- GitHub Flow : branches `feat/<sujet>`, une PR par changement, Conventional Commits.
- Avant de pousser : `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (dans `src-tauri/`), puis `npm run lint && npm run typecheck && npm test`.
- Fixtures : uniquement de fausses données.
- Hooks : `lefthook install`.
