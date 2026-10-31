import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";

export function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <main className="flex min-h-screen items-center justify-center bg-zinc-950 p-6 text-zinc-100">
      <div className="w-full max-w-md space-y-5 rounded-2xl border border-zinc-800 bg-zinc-900 p-8">
        <h1 className="text-2xl font-semibold">{title}</h1>
        {children}
      </div>
    </main>
  );
}

export function Field({
  label,
  ...props
}: { label: string } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="block space-y-1 text-sm">
      <span className="text-zinc-300">{label}</span>
      <input
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        className="w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-emerald-500 focus-visible:ring-2 focus-visible:ring-emerald-500/60"
        {...props}
      />
    </label>
  );
}

export function Button({
  variant = "primary",
  ...props
}: { variant?: "primary" | "ghost" } & ButtonHTMLAttributes<HTMLButtonElement>) {
  const base =
    "rounded-lg px-4 py-2 text-sm font-medium transition focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-400 disabled:cursor-not-allowed disabled:opacity-50";
  const styles =
    variant === "primary"
      ? "bg-emerald-600 text-white hover:bg-emerald-500"
      : "text-zinc-300 hover:bg-zinc-800";
  return <button className={`${base} ${styles}`} {...props} />;
}

export function ErrorText({ children }: { children?: ReactNode }) {
  return children ? (
    <p role="alert" className="text-sm text-red-400">
      {children}
    </p>
  ) : null;
}
