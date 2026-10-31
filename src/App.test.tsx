import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import App from "./App";

function renderApp() {
  return render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <App />
    </QueryClientProvider>,
  );
}

afterEach(async () => {
  cleanup();
  // laisse les désabonnements d'événements (asynchrones) se terminer avant de retirer le mock
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
});

describe("parcours du coffre", () => {
  it("propose la création quand aucun coffre n'existe", async () => {
    mockIPC(
      (cmd) =>
        cmd === "vault_status" ? { exists: false, unlocked: false, auto_lock_minutes: 5 } : null,
      { shouldMockEvents: true },
    );
    renderApp();
    expect(await screen.findByText("Créer ton coffre")).toBeInTheDocument();
  });

  it("affiche l'écran de déverrouillage quand le coffre est verrouillé", async () => {
    mockIPC(
      (cmd) =>
        cmd === "vault_status" ? { exists: true, unlocked: false, auto_lock_minutes: 5 } : null,
      { shouldMockEvents: true },
    );
    renderApp();
    expect(await screen.findByText("Vigie est verrouillé")).toBeInTheDocument();
  });

  it("montre la clé de récupération une fois, puis exige la confirmation", async () => {
    let created = false;
    mockIPC(
      (cmd) => {
        if (cmd === "vault_status")
          return { exists: created, unlocked: created, auto_lock_minutes: 5 };
        if (cmd === "password_strength") return 3;
        if (cmd === "create_vault") {
          created = true;
          return "AAAAA-BBBBB-CCCCC";
        }
        return null;
      },
      { shouldMockEvents: true },
    );
    const user = userEvent.setup();
    renderApp();
    await user.type(
      await screen.findByLabelText("Mot de passe maître"),
      "une phrase de passe solide",
    );
    await user.type(screen.getByLabelText("Confirmation"), "une phrase de passe solide");
    await user.click(screen.getByRole("button", { name: "Créer le coffre" }));

    expect(await screen.findByTestId("recovery-key")).toHaveTextContent("AAAAA-BBBBB-CCCCC");
    const next = screen.getByRole("button", { name: "Continuer" });
    expect(next).toBeDisabled();
    await user.click(screen.getByRole("checkbox"));
    await user.click(next);
    await waitFor(() => expect(screen.getByText("Réglages")).toBeInTheDocument());
  });
});
