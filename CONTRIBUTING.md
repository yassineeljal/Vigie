# Contribuer

- GitHub Flow : branches `feat/US-xx-…`, une PR par story, Conventional Commits.
- Avant de pousser : `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (dans `src-tauri/`), puis `npm run lint && npm run typecheck && npm test`.
- Fixtures : uniquement de fausses données.
- Hooks : `lefthook install`.
