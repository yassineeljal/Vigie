-- Une seule action par (compte, type) : l'état (à faire / fait / ignoré) vit sur la ligne.
CREATE UNIQUE INDEX idx_action_unique ON action(account_id, kind);
