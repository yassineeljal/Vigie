import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { LangProvider } from "./i18n";
import "./index.css";

const queryClient = new QueryClient();

/** Affiche une erreur de démarrage au lieu d'une fenêtre blanche (aide au diagnostic). */
function showFatal(err: unknown) {
  const root = document.getElementById("root");
  if (!root) return;
  const msg =
    err instanceof Error ? `${err.name}: ${err.message}\n${err.stack ?? ""}` : String(err);
  root.textContent = "";
  const pre = document.createElement("pre");
  pre.style.cssText =
    "padding:16px;color:#f87171;background:#09090b;white-space:pre-wrap;min-height:100vh";
  pre.textContent = `Vigie n'a pas pu démarrer.\n\n${msg}`;
  root.appendChild(pre);
}

class Boundary extends React.Component<{ children: React.ReactNode }, { error: unknown }> {
  state = { error: null as unknown };
  static getDerivedStateFromError(error: unknown) {
    return { error };
  }
  render() {
    if (this.state.error) {
      const e = this.state.error;
      return (
        <pre className="min-h-screen bg-bg p-4 whitespace-pre-wrap text-danger">
          {e instanceof Error ? `${e.name}: ${e.message}\n${e.stack ?? ""}` : String(e)}
        </pre>
      );
    }
    return this.props.children;
  }
}

window.addEventListener("error", (e) => showFatal(e.error ?? e.message));
window.addEventListener("unhandledrejection", (e) => showFatal(e.reason));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Boundary>
      <LangProvider>
        <QueryClientProvider client={queryClient}>
          <App />
        </QueryClientProvider>
      </LangProvider>
    </Boundary>
  </React.StrictMode>,
);
