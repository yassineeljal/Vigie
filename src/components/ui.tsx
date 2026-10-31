import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";

/** Repère de la marque : une vigie, cercle de guet traversé par la ligne d'horizon. */
export function Mark({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true" className="text-primary">
      <circle cx="16" cy="16" r="13" fill="none" stroke="currentColor" strokeWidth="2.5" />
      <path d="M3.5 18h25" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
      <circle cx="16" cy="11" r="3" fill="currentColor" />
    </svg>
  );
}

export function Wordmark() {
  return (
    <span className="flex items-center gap-2">
      <Mark />
      <span className="font-display text-2xl leading-none text-ink">Vigie</span>
    </span>
  );
}

/** Écran plein cadre pour l'accueil, la création et le déverrouillage. */
export function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-8 bg-bg p-6">
      <Wordmark />
      <div className="w-full max-w-md space-y-5 rounded-xl border border-line bg-surface p-8">
        <h1 className="font-display text-3xl leading-tight text-ink">{title}</h1>
        {children}
      </div>
    </main>
  );
}

export function PageHeader({
  title,
  children,
  aside,
}: {
  title: string;
  children?: ReactNode;
  aside?: ReactNode;
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-5">
      <div className="max-w-2xl space-y-1">
        <h2 className="font-display text-4xl leading-none text-ink">{title}</h2>
        {children && <p className="text-sm text-muted">{children}</p>}
      </div>
      {aside}
    </header>
  );
}

export function Field({
  label,
  hint,
  ...props
}: { label: string; hint?: string } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="block space-y-1.5 text-sm">
      <span className="font-medium text-ink">{label}</span>
      <input
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        className="w-full rounded-md border border-line bg-surface px-3 py-2 text-ink placeholder:text-muted focus:border-primary"
        {...props}
      />
      {hint && <span className="block text-xs text-muted">{hint}</span>}
    </label>
  );
}

type Variant = "primary" | "secondary" | "ghost" | "danger";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-primary text-on-primary hover:opacity-90",
  secondary: "border border-line bg-surface text-ink hover:bg-sunken",
  ghost: "border border-line text-ink hover:bg-sunken",
  danger: "bg-danger text-white hover:opacity-90",
};

export function Button({
  variant = "primary",
  ...props
}: { variant?: Variant } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={`inline-flex items-center justify-center gap-2 rounded-md px-3.5 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${VARIANTS[variant]}`}
      {...props}
    />
  );
}

export const selectClass =
  "rounded-md border border-line bg-surface px-2.5 py-2 text-sm text-ink focus:border-primary";

export function ErrorText({ children }: { children?: ReactNode }) {
  return children ? (
    <p role="alert" className="text-sm text-danger">
      {children}
    </p>
  ) : null;
}

type Tone = "neutral" | "safe" | "warn" | "danger";
const TONES: Record<Tone, string> = {
  neutral: "bg-sunken text-muted",
  safe: "bg-safe-bg text-safe",
  warn: "bg-warn-bg text-warn",
  danger: "bg-danger-bg text-danger",
};

export function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${TONES[tone]}`}
    >
      {children}
    </span>
  );
}

export function Panel({
  title,
  action,
  children,
}: {
  title?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="space-y-3 rounded-xl border border-line bg-surface p-5">
      {(title || action) && (
        <div className="flex items-baseline justify-between gap-3">
          {title && <h3 className="text-sm font-semibold text-ink">{title}</h3>}
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return (
    <p className="rounded-lg border border-dashed border-line px-4 py-8 text-center text-sm text-muted">
      {children}
    </p>
  );
}
