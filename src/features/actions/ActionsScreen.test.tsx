import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it, vi } from "vitest";
import { ActionsScreen } from "./ActionsScreen";

afterEach(() => {
  cleanup();
  clearMocks();
});

const action = (over: object) => ({
  id: 1,
  account_id: 1,
  account_label: "Banque",
  domain: "banque.ca",
  kind: "change_password",
  gain: 45,
  effort_min: 5,
  ratio: 9,
  status: "todo",
  factors: ["pwned_password"],
  supported_mfa: [],
  supports_passkeys: false,
  ...over,
});

it("affiche gain, effort et facteur corrigé, bascule en quick wins et complète une action MFA", async () => {
  const listCalls = vi.fn();
  const complete = vi.fn();
  mockIPC((cmd, args) => {
    if (cmd === "list_actions") {
      listCalls(args);
      return [
        action({}),
        action({
          id: 2,
          account_id: 2,
          account_label: "Gmail",
          domain: "gmail.com",
          kind: "enable_mfa",
          gain: 25,
          effort_min: 5,
          ratio: 5,
          factors: ["no_mfa"],
          supports_passkeys: true,
        }),
      ];
    }
    if (cmd === "get_score_report") return { rules_version: "v1", global: 62.4, accounts: [] };
    if (cmd === "complete_action") {
      complete(args);
      return { rules_version: "v1", global: 80, accounts: [] };
    }
    return null;
  });
  const user = userEvent.setup();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ActionsScreen />
    </QueryClientProvider>,
  );
  expect(
    await screen.findByText(/Corrige : Mot de passe présent dans des fuites/),
  ).toBeInTheDocument();
  expect(screen.getByText("−45.0")).toBeInTheDocument();
  expect(screen.getByText(/Ce site supporte les passkeys/)).toBeInTheDocument();
  expect(await screen.findByLabelText("Score global 62 sur 100")).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Quick wins" }));
  await waitFor(() =>
    expect(listCalls).toHaveBeenCalledWith({ view: "quick_wins", status: "todo" }),
  );

  // La méthode proposée par défaut est la passkey (le site la supporte).
  expect(screen.getByLabelText("Méthode MFA activée pour Gmail")).toHaveValue("passkey");
  await user.click(screen.getAllByRole("button", { name: "Marquer comme fait" })[1]);
  await waitFor(() => expect(complete).toHaveBeenCalledWith({ id: 2, mfaKind: "passkey" }));
});
