-- Un contrôle importé (force + groupe de réutilisation) n'a pas encore de résultat de fuite :
-- breach_count devient nullable (NULL = jamais vérifié auprès de Pwned Passwords).
DROP TABLE password_check;
CREATE TABLE password_check (
  id           INTEGER PRIMARY KEY,
  account_id   INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  checked_at   TEXT NOT NULL,
  breach_count INTEGER,
  strength     INTEGER CHECK (strength BETWEEN 0 AND 4),
  reuse_group  INTEGER                      -- identifiant de groupe, jamais le mot de passe
) STRICT;
CREATE INDEX idx_password_check_account ON password_check(account_id);
