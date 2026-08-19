import React, { useState } from "react";

export const Card: React.FC<{ title?: string; children: React.ReactNode; style?: React.CSSProperties }> = ({
  title,
  children,
  style,
}) => (
  <div className="card" style={style}>
    {title && <h2 className="card-title">{title}</h2>}
    {children}
  </div>
);

export const Stat: React.FC<{ label: string; value: React.ReactNode; sub?: string; color?: string }> = ({
  label,
  value,
  sub,
  color,
}) => (
  <Card title={label}>
    <div className="stat-big" style={color ? { color } : undefined}>{value}</div>
    {sub && <div className="stat-sub">{sub}</div>}
  </Card>
);

export const Badge: React.FC<{ cls: string; children: React.ReactNode }> = ({ cls, children }) => (
  <span className={`badge ${cls}`}>{children}</span>
);

export const Spinner = () => <span className="spinner" />;

export const Bar: React.FC<{ pct: number; color?: string }> = ({ pct, color }) => (
  <div className="bar">
    <div style={{ width: `${Math.min(100, Math.max(0, pct))}%`, ...(color ? { background: color } : {}) }} />
  </div>
);

/** Raw-JSON disclosure for expert mode. */
export const RawJson: React.FC<{ label?: string; data: any }> = ({ label = "Raw data", data }) => (
  <details>
    <summary>{label}</summary>
    <pre className="mono muted" style={{ maxHeight: 280, overflow: "auto", marginTop: 6 }}>
      {JSON.stringify(data, null, 2)}
    </pre>
  </details>
);

/** Simple async-action button with busy state and error surfacing. */
export const ActionBtn: React.FC<{
  label: string;
  onRun: () => Promise<void>;
  className?: string;
  disabled?: boolean;
}> = ({ label, onRun, className = "btn", disabled }) => {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <>
      <button
        className={className}
        disabled={busy || disabled}
        onClick={async () => {
          setBusy(true);
          setError(null);
          try {
            await onRun();
          } catch (e: any) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        {busy ? <Spinner /> : label}
      </button>
      {error && <ErrorBanner msg={error} />}
    </>
  );
};

/** Inline error banner with optional retry. Never swallow a failure silently. */
export const ErrorBanner: React.FC<{ msg?: string | null; onRetry?: () => void }> = ({ msg, onRetry }) => {
  if (!msg) return null;
  return (
    <div
      role="alert"
      className="warn-banner"
      style={{ borderColor: "var(--red)", color: "var(--red)", background: "rgba(255,97,125,.08)" }}
    >
      <b>⚠ </b>
      {msg}
      {onRetry && (
        <button className="btn small ghost" style={{ marginLeft: 8 }} onClick={onRetry} title="Retry">
          ↻
        </button>
      )}
    </div>
  );
};
