import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it, vi } from "vitest";
import { ImportScreen } from "./ImportScreen";

afterEach(() => {
  cleanup();
  clearMocks();
});

const item = (over: object) => ({
  index: 0,
  label: "Gmail",
  domain: "google.com",
  username: "a@x.test",
  has_totp: true,
  has_password: true,
  strength: 3,
  duplicate_of: null,
  repeats_index: null,
  reuse_group: null,
  issue: null,
  ...over,
});

it("parcourt l'assistant : choix, aperçu avec réutilisation, import, avertissement de suppression", async () => {
  const commit = vi.fn();
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "import_pick_file":
        return { name: "export.csv", size_bytes: 2048 };
      case "import_inspect":
        return { format: "bitwarden_csv", headers: [] };
      case "import_start":
        return {
          items: [
            item({}),
            item({ index: 1, label: "Instagram", reuse_group: 1, strength: 0 }),
            item({ index: 2, label: "Facebook", reuse_group: 1 }),
            item({ index: 3, label: "App", domain: null, issue: "URL ou domaine invalide" }),
          ],
          reuse_groups: 1,
          reused_accounts: 2,
          skipped_rows: 0,
        };
      case "import_commit":
        commit(args);
        return {
          report: {
            created: 3,
            merged: 0,
            skipped_duplicates: 0,
            skipped_invalid: 1,
            excluded: 0,
            reuse_groups: 1,
            reused_accounts: 2,
          },
          file_path: "/Users/moi/Downloads/export.csv",
        };
      default:
        return null;
    }
  });
  const user = userEvent.setup();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ImportScreen />
    </QueryClientProvider>,
  );
  await user.click(screen.getByRole("button", { name: /Choisir un fichier/ }));
  expect(await screen.findByText(/Format détecté : Bitwarden \(CSV\)/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Analyser" }));

  expect(
    await screen.findByText(/2 compte\(s\) avec un mot de passe réutilisé \(1 groupe/),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Importer App")).toBeDisabled();
  await user.click(screen.getByLabelText("Importer Facebook")); // exclu
  await user.click(screen.getByRole("button", { name: "Importer 2 compte(s)" }));

  await waitFor(() => expect(commit).toHaveBeenCalled());
  expect(commit.mock.calls[0][0]).toMatchObject({
    policy: "merge",
    excluded: expect.arrayContaining([2, 3]),
  });
  expect(await screen.findByText(/Supprime maintenant ton fichier/)).toBeInTheDocument();
  expect(screen.getByText("/Users/moi/Downloads/export.csv")).toBeInTheDocument();
});
