import React from "react";

// ── Button ──

type ButtonVariant = "primary" | "danger" | "ghost";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  loading?: boolean;
  size?: "sm" | "md";
}

const variants: Record<ButtonVariant, React.CSSProperties> = {
  primary: { background: "#1a1a2e", color: "#fff" },
  danger: { background: "#dc3545", color: "#fff" },
  ghost: { background: "transparent", color: "#1a1a2e", border: "1px solid #ddd" },
};

export function Button({ variant = "primary", loading, size = "md", style, children, ...props }: ButtonProps) {
  return (
    <button
      style={{
        padding: size === "sm" ? "4px 10px" : "8px 16px",
        borderRadius: 6,
        border: "none",
        cursor: "pointer",
        fontWeight: 500,
        fontSize: size === "sm" ? 12 : 14,
        opacity: props.disabled || loading ? 0.6 : 1,
        ...variants[variant],
        ...style,
      }}
      {...props}
    >
      {loading ? "⏳" : children}
    </button>
  );
}

// ── Input ──

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export function Input({ label, error, style, ...props }: InputProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {label && <label style={{ fontSize: 13, fontWeight: 500, color: "#555" }}>{label}</label>}
      <input
        style={{
          padding: "8px 12px",
          borderRadius: 6,
          border: `1px solid ${error ? "#dc3545" : "#ccc"}`,
          fontSize: 14,
          outline: "none",
          ...style,
        }}
        {...props}
      />
      {error && <span style={{ fontSize: 12, color: "#dc3545" }}>{error}</span>}
    </div>
  );
}

// ── Badge ──

interface BadgeProps {
  variant?: "success" | "danger" | "info" | "warning";
  children: React.ReactNode;
}

const badgeColors: Record<string, React.CSSProperties> = {
  success: { background: "#e8f5e9", color: "#2e7d32" },
  danger: { background: "#fce4ec", color: "#c62828" },
  info: { background: "#e3f2fd", color: "#1565c0" },
  warning: { background: "#fff3cd", color: "#856404" },
};

export function Badge({ variant = "info", children }: BadgeProps) {
  return (
    <span
      style={{
        padding: "2px 8px",
        borderRadius: 4,
        fontSize: 12,
        fontWeight: 500,
        ...badgeColors[variant],
      }}
    >
      {children}
    </span>
  );
}

// ── Modal ──

interface ModalProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  variant?: ButtonVariant;
  onConfirm: () => void;
  onCancel: () => void;
  loading?: boolean;
}

export function Modal({ open, title, message, confirmLabel = "Confirm", variant = "danger", onConfirm, onCancel, loading }: ModalProps) {
  if (!open) return null;
  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}>
      <div style={{ background: "#fff", borderRadius: 12, padding: 24, minWidth: 360, boxShadow: "0 4px 24px rgba(0,0,0,0.15)" }}>
        <h3 style={{ marginTop: 0 }}>{title}</h3>
        <p style={{ color: "#666", fontSize: 14 }}>{message}</p>
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 20 }}>
          <Button variant="ghost" onClick={onCancel}>Cancel</Button>
          <Button variant={variant} onClick={onConfirm} loading={loading}>{confirmLabel}</Button>
        </div>
      </div>
    </div>
  );
}

// ── Toast ──

let toastId = 0;

export function toast(message: string, type: "success" | "error" = "success") {
  ++toastId;
  const el = document.createElement("div");
  el.style.cssText = `
    position: fixed; top: 16px; right: 16px; z-index: 2000;
    padding: 12px 20px; border-radius: 8px; color: #fff; font-size: 14px;
    animation: slideIn 0.3s ease;
    background: ${type === "error" ? "#dc3545" : "#2e7d32"};
  `;
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => { el.remove(); }, 3000);
}

// ── Table ──

interface Column<T> {
  key: string;
  header: string;
  width?: number;
  render?: (row: T) => React.ReactNode;
}

interface TableProps<T> {
  columns: Column<T>[];
  data: T[];
  rowKey: (row: T) => string;
  emptyText?: string;
}

export function Table<T>({ columns, data, rowKey, emptyText = "No data" }: TableProps<T>) {
  if (data.length === 0) {
    return <p style={{ color: "#888", marginTop: 24 }}>{emptyText}</p>;
  }
  return (
    <table style={{ width: "100%", borderCollapse: "collapse", marginTop: 16 }}>
      <thead>
        <tr style={{ borderBottom: "2px solid #e0e0e0" }}>
          {columns.map((col) => (
            <th key={col.key} style={{ textAlign: "left", padding: "10px 12px", fontSize: 13, fontWeight: 600, color: "#555", width: col.width }}>
              {col.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {data.map((row) => (
          <tr key={rowKey(row)} style={{ borderBottom: "1px solid #f0f0f0" }}>
            {columns.map((col) => (
              <td key={col.key} style={{ padding: "10px 12px", fontSize: 14 }}>
                {col.render ? col.render(row) : String((row as any)[col.key] ?? "")}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
