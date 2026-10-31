import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it, vi } from "vitest";
import { AccountForm } from "./AccountForm";

afterEach(() => {
  cleanup();
  clearMocks();
});

it("prévisualise le domaine normalisé et envoie le compte saisi", async () => {
  const saved = vi.fn();
  mockIPC((cmd, args) => {
    if (cmd === "preview_domain") return "exemple.co.uk";
    if (cmd === "list_identities") return [];
    if (cmd === "save_account") {
      saved(args);
      return {};
    }
    return null;
  });
  const onClose = vi.fn();
  const user = userEvent.setup();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <AccountForm account={null} onClose={onClose} />
    </QueryClientProvider>,
  );
  await user.type(screen.getByLabelText("Site (URL ou domaine)"), "https://login.exemple.co.uk/x");
  expect(await screen.findByTestId("domain-preview")).toHaveTextContent("exemple.co.uk");
  await user.type(screen.getByLabelText("Libellé"), "Exemple");
  await user.click(screen.getByLabelText("TOTP"));
  await user.type(screen.getByLabelText(/Étiquettes/), "perso, dev");
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));

  await waitFor(() => expect(onClose).toHaveBeenCalled());
  const input = saved.mock.calls[0][0].input;
  expect(input).toMatchObject({
    domain: "https://login.exemple.co.uk/x",
    label: "Exemple",
    tags: ["perso", "dev"],
    mfa: [{ kind: "totp", backup_codes: false }],
  });
});
