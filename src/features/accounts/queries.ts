import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AccountFilter, type AccountInput, type IdentityInput } from "../../lib/ipc";
import { unwrap } from "../../lib/result";

export const DEFAULT_FILTER: AccountFilter = {
  search: null,
  status: null,
  criticality: null,
  no_mfa: false,
  tag: null,
  sort: "label",
  descending: false,
};

export function useAccounts(filter: AccountFilter) {
  return useQuery({
    queryKey: ["accounts", filter],
    queryFn: () => unwrap(commands.listAccounts(filter)),
    placeholderData: (prev) => prev,
  });
}

export const useTags = () =>
  useQuery({ queryKey: ["tags"], queryFn: () => unwrap(commands.listTags()) });

export const useIdentities = () =>
  useQuery({ queryKey: ["identities"], queryFn: () => unwrap(commands.listIdentities()) });

function useInvalidating<V, R>(fn: (v: V) => Promise<R>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => {
      for (const key of [
        "accounts",
        "tags",
        "identities",
        "score",
        "actions",
        "dashboard",
        "graph",
      ]) {
        void qc.invalidateQueries({ queryKey: [key] });
      }
    },
  });
}

export const useSaveAccount = () =>
  useInvalidating(({ id, input }: { id: number | null; input: AccountInput }) =>
    unwrap(commands.saveAccount(id, input)),
  );
export const useDeleteAccount = () =>
  useInvalidating((id: number) => unwrap(commands.deleteAccount(id)));
export const useSaveIdentity = () =>
  useInvalidating(({ id, input }: { id: number | null; input: IdentityInput }) =>
    unwrap(commands.saveIdentity(id, input)),
  );
export const useDeleteIdentity = () =>
  useInvalidating((id: number) => unwrap(commands.deleteIdentity(id)));
