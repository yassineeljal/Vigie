-- Schéma v1 (docs/PLAN.md §5.4). Aucune colonne ne stocke de mot de passe ni de hash de mot de passe.
CREATE TABLE service (
  domain        TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  supported_mfa TEXT,                       -- JSON, depuis 2fa.directory
  passkeys      INTEGER NOT NULL DEFAULT 0,
  synced_at     TEXT
) STRICT;

CREATE TABLE identity (
  id         INTEGER PRIMARY KEY,
  kind       TEXT NOT NULL CHECK (kind IN ('email', 'phone')),
  value      TEXT NOT NULL,
  account_id INTEGER REFERENCES account(id) ON DELETE SET NULL,
  UNIQUE (kind, value)
) STRICT;

CREATE TABLE account (
  id                    INTEGER PRIMARY KEY,
  domain                TEXT NOT NULL REFERENCES service(domain),
  label                 TEXT NOT NULL,
  identity_id           INTEGER REFERENCES identity(id) ON DELETE SET NULL,
  username              TEXT,
  criticality           TEXT NOT NULL DEFAULT 'medium'
                        CHECK (criticality IN ('critical', 'high', 'medium', 'low')),
  recovery_identity_id  INTEGER REFERENCES identity(id) ON DELETE SET NULL,
  pwd_changed_at        TEXT,
  status                TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'unused', 'deleted')),
  notes                 TEXT,
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL
) STRICT;
CREATE INDEX idx_account_domain ON account(domain);

CREATE TABLE account_tag (
  account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  tag        TEXT NOT NULL,
  PRIMARY KEY (account_id, tag)
) STRICT;

CREATE TABLE mfa_method (
  id           INTEGER PRIMARY KEY,
  account_id   INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('passkey', 'hardware_key', 'totp', 'push', 'sms', 'email')),
  backup_codes INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE password_check (
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  checked_at  TEXT NOT NULL,
  breach_count INTEGER NOT NULL,
  strength    INTEGER NOT NULL CHECK (strength BETWEEN 0 AND 4),
  reuse_group INTEGER                       -- identifiant de groupe, jamais le mot de passe
) STRICT;

CREATE TABLE breach (
  name         TEXT PRIMARY KEY,            -- nom HIBP
  title        TEXT NOT NULL,
  domain       TEXT,
  breach_date  TEXT,
  added_date   TEXT,
  data_classes TEXT NOT NULL,               -- JSON
  verified     INTEGER NOT NULL DEFAULT 0,
  sensitive    INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX idx_breach_domain ON breach(domain);

CREATE TABLE breach_match (
  breach_name     TEXT NOT NULL REFERENCES breach(name) ON DELETE CASCADE,
  account_id      INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  kind            TEXT NOT NULL CHECK (kind IN ('domain', 'email')),
  relevant        INTEGER NOT NULL DEFAULT 1,
  acknowledged_at TEXT,
  PRIMARY KEY (breach_name, account_id, kind)
) STRICT;

CREATE TABLE action (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  kind          TEXT NOT NULL,
  gain          REAL NOT NULL,
  effort_min    INTEGER NOT NULL,
  status        TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'done', 'ignored')),
  rules_version TEXT NOT NULL,
  created_at    TEXT NOT NULL,
  done_at       TEXT
) STRICT;

CREATE TABLE score_snapshot (
  id            INTEGER PRIMARY KEY,
  taken_at      TEXT NOT NULL,
  global_score  REAL NOT NULL,
  rules_version TEXT NOT NULL,
  details       TEXT NOT NULL               -- JSON
) STRICT;

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL                       -- jamais de secret
) STRICT;
