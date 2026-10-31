import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it, vi } from "vitest";
import { BreachesScreen } from "./BreachesScreen";

afterEach(async () => {
  cleanup();
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
});

it("vérifie un mot de passe puis vide le champ immédiatement", async () => {
  const check = vi.fn();
  mockIPC(
    (cmd, args) => {
      switch (cmd) {
        case "intel_status":
          return {
            offline_mode: false,
            breaches_synced_at: "2026-10-31T12:00:00Z",
            breach_count: 800,
            has_api_key: false,
          };
        case "list_breach_matches":
          return [
            {
              breach_name: "Exemple",
              title: "Exemple",
              breach_date: "2024-05-02",
              data_classes: ["Passwords"],
              account_id: 1,
              account_label: "Exemple.com",
              domain: "exemple.com",
              kind: "domain",
              relevant: true,
              acknowledged_at: null,
            },
          ];
        case "check_password":
          check(args);
          return { breach_count: 9545824, strength: 0 };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  const user = userEvent.setup();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <BreachesScreen />
    </QueryClientProvider>,
  );
  expect(await screen.findByText(/Brèche postérieure au dernier changement/)).toBeInTheDocument();
  expect(screen.getByText(/800 brèches/)).toBeInTheDocument();

  const field = screen.getByLabelText("Mot de passe");
  await user.type(field, "password");
  await user.click(screen.getByRole("button", { name: "Vérifier" }));
  expect(field).toHaveValue("");
  await waitFor(() =>
    expect(check).toHaveBeenCalledWith({ password: "password", accountId: null }),
  );
  expect(await screen.findByRole("status")).toHaveTextContent(/Trouvé 9\s?545\s?824 fois/);
});
