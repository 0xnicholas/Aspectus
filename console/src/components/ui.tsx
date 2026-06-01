import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../lib/utils";

// ── Button (Metronic-style variants) ──

const buttonVariants = cva(
  "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
  {
    variants: {
      variant: {
        primary: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-red-600 text-white hover:bg-red-700",
        outline: "border border-border bg-white hover:bg-accent",
        ghost: "text-gray-600 hover:bg-accent hover:text-gray-900",
      },
      size: {
        sm: "h-8 px-3 text-xs",
        md: "h-10 px-4",
        lg: "h-12 px-6",
      },
    },
    defaultVariants: {
      variant: "primary",
      size: "md",
    },
  }
);

interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export function Button({ className, variant, size, loading, children, ...props }: ButtonProps) {
  return (
    <button className={cn(buttonVariants({ variant, size }), className)} {...props}>
      {loading && <span className="mr-2 animate-spin">⏳</span>}
      {children}
    </button>
  );
}

// ── Input ──

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export function Input({ label, error, className, ...props }: InputProps) {
  return (
    <div className="flex flex-col gap-1">
      {label && <label className="text-sm font-medium text-gray-600">{label}</label>}
      <input
        className={cn(
          "h-10 rounded-md border border-border px-3 text-sm outline-none focus:border-primary focus:ring-1 focus:ring-primary",
          error && "border-red-500",
          className
        )}
        {...props}
      />
      {error && <span className="text-xs text-red-500">{error}</span>}
    </div>
  );
}

// ── Badge (Metronic-style) ──

const badgeVariants = cva("inline-flex items-center rounded px-2 py-0.5 text-xs font-medium", {
  variants: {
    variant: {
      success: "bg-green-100 text-green-800",
      destructive: "bg-red-100 text-red-800",
      warning: "bg-yellow-100 text-yellow-800",
      info: "bg-blue-100 text-blue-800",
    },
  },
  defaultVariants: { variant: "info" },
});

interface BadgeProps extends VariantProps<typeof badgeVariants> {
  children: React.ReactNode;
}

export function Badge({ variant, children }: BadgeProps) {
  return <span className={badgeVariants({ variant })}>{children}</span>;
}

// ── Table (Tailwind styled) ──

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
    return <p className="mt-6 text-gray-400">{emptyText}</p>;
  }
  return (
    <div className="mt-4 overflow-x-auto rounded-lg border border-border bg-white">
      <table className="w-full text-sm">
        <thead className="border-b border-border bg-gray-50">
          <tr>
            {columns.map((col) => (
              <th key={col.key} className="px-4 py-3 text-left font-medium text-gray-500" style={{ width: col.width }}>
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.map((row) => (
            <tr key={rowKey(row)} className="border-b border-border last:border-0 hover:bg-gray-50">
              {columns.map((col) => (
                <td key={col.key} className="px-4 py-3">
                  {col.render ? col.render(row) : String((row as any)[col.key] ?? "")}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Modal (Metronic-style dialog) ──

interface ModalProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  variant?: "primary" | "destructive";
  onConfirm: () => void;
  onCancel: () => void;
  loading?: boolean;
}

export function Modal({ open, title, message, confirmLabel = "Confirm", variant = "destructive", onConfirm, onCancel, loading }: ModalProps) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl bg-white p-6 shadow-xl">
        <h3 className="text-lg font-semibold">{title}</h3>
        <p className="mt-2 text-sm text-gray-500">{message}</p>
        <div className="mt-6 flex justify-end gap-3">
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
  el.className = `fixed top-4 right-4 z-50 px-4 py-3 rounded-lg text-white text-sm shadow-lg animate-[slideIn_0.3s_ease] ${
    type === "error" ? "bg-red-600" : "bg-green-600"
  }`;
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 3000);
}
