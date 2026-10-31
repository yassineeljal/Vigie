import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it } from "vitest";
import { DashboardScreen } from "./DashboardScreen";

afterEach(() => {
  cleanup();
  clearMocks();
});

it("affiche le score, les indicateurs et les actions prioritaires", async () => {
  mockIPC((cmd) =>
    cmd === "get_dashboard"
      ? {
          score: 71.6,
          delta_30d: -2.5,
          accounts_total: 42,
          without_mfa: 7,
          critical_without_strong_mfa: 2,
          by_criticality: [{ criticality: "critical", count: 5 }],
          risk_buckets: [{ level: "severe", count: 3 }],
          top_factors: [{ factor: "no_mfa", accounts: 7 }],
          recent_breaches: [],
          top_actions: [
            {
              id: 1,
              account_id: 1,
              account_label: "Gmail",
              domain: "gmail.com",
              kind: "enable_mfa",
              gain: 25,
              effort_min: 5,
              ratio: 5,
              status: "todo",
              factors: ["no_mfa"],
              supported_mfa: [],
              supports_passkeys: false,
            },
          ],
          pending_actions: 9,
          potential_gain: 130.4,
          history: [],
        }
      : null,
  );
  render(
    <QueryClientProvider client={new QueryClient()}>
      <DashboardScreen onNavigate={() => {}} />
    </QueryClientProvider>,
  );
  expect(await screen.findByLabelText("Score 72 sur 100")).toBeInTheDocument();
  expect(screen.getByText("▼ 2.5 sur 30 jours")).toBeInTheDocument();
  expect(screen.getByText("Critiques sans MFA forte")).toBeInTheDocument();
  expect(screen.getByText("Gmail")).toBeInTheDocument();
  expect(screen.getByText(/9 en attente · gain potentiel 130/)).toBeInTheDocument();
  expect(screen.getByText(/L'historique se construit jour après jour/)).toBeInTheDocument();
});
