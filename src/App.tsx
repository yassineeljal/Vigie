import { useQuery } from "@tanstack/react-query";
import { commands } from "./lib/ipc";

export default function App() {
  const ping = useQuery({ queryKey: ["ping"], queryFn: () => commands.ping() });

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-2 bg-zinc-950 text-zinc-100">
      <h1 className="text-3xl font-semibold">Vigie</h1>
      <p className="text-sm text-zinc-400">
        {ping.isSuccess
          ? `Cœur Rust connecté (v${ping.data.version})`
          : ping.isError
            ? "Cœur Rust injoignable"
            : "Connexion au cœur Rust…"}
      </p>
    </main>
  );
}
