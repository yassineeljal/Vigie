import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import App from "./App";

afterEach(() => clearMocks());

it("affiche la version retournée par la commande ping", async () => {
  mockIPC((cmd) => (cmd === "ping" ? { message: "pong", version: "9.9.9" } : null));
  render(
    <QueryClientProvider client={new QueryClient()}>
      <App />
    </QueryClientProvider>,
  );
  expect(await screen.findByText(/v9\.9\.9/)).toBeInTheDocument();
});
