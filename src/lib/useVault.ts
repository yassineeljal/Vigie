import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, events } from "./ipc";

export const VAULT_KEY = ["vault-status"] as const;

export function useVaultStatus() {
  return useQuery({ queryKey: VAULT_KEY, queryFn: () => commands.vaultStatus() });
}

/**
 * Rafraîchit l'état quand le cœur verrouille le coffre (inactivité) et signale
 * l'activité de l'utilisateur, au plus une fois toutes les 5 s (US-02).
 */
export function useVaultLifecycle(unlocked: boolean) {
  const qc = useQueryClient();

  useEffect(() => {
    const un = events.vaultLocked.listen(() => {
      // Aucune donnée d'inventaire ne doit rester dans le cache après verrouillage.
      qc.removeQueries({ predicate: (q) => q.queryKey[0] !== VAULT_KEY[0] });
      void qc.invalidateQueries({ queryKey: VAULT_KEY });
    });
    return () => void un.then((f) => f());
  }, [qc]);

  useEffect(() => {
    if (!unlocked) return;
    let last = 0;
    const onActivity = () => {
      const now = Date.now();
      if (now - last < 5000) return;
      last = now;
      void commands.touchActivity();
    };
    const names = ["keydown", "pointerdown", "wheel"] as const;
    names.forEach((n) => window.addEventListener(n, onActivity, { passive: true }));
    return () => names.forEach((n) => window.removeEventListener(n, onActivity));
  }, [unlocked]);
}
