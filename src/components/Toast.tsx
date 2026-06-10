import { useEffect } from "react";
import { CheckCircle, AlertCircle, Info, X } from "lucide-react";
import type { Toast as ToastType } from "../types";

interface ToastProps {
  toasts: ToastType[];
  onDismiss: (id: string) => void;
}

const ICONS = {
  success: CheckCircle,
  error: AlertCircle,
  info: Info,
};

function ToastItem({
  toast,
  onDismiss,
}: {
  toast: ToastType;
  onDismiss: () => void;
}) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, 5000);
    return () => clearTimeout(timer);
  }, [onDismiss]);

  const Icon = ICONS[toast.type];

  return (
    <div className={`toast toast-${toast.type}`}>
      <Icon size={16} />
      <span className="toast-message">{toast.message}</span>
      <button className="toast-close" onClick={onDismiss}>
        <X size={14} />
      </button>
    </div>
  );
}

export function ToastContainer({ toasts, onDismiss }: ToastProps) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <ToastItem
          key={toast.id}
          toast={toast}
          onDismiss={() => onDismiss(toast.id)}
        />
      ))}
    </div>
  );
}
