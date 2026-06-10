import { AlertTriangle } from "lucide-react";

interface ConfirmDialogProps {
  title: string;
  message: string;
  detail?: string;
  confirmLabel: string;
  cancelLabel?: string;
  variant?: "danger" | "warning" | "default";
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  detail,
  confirmLabel,
  cancelLabel = "Cancel",
  variant = "default",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-icon">
          <AlertTriangle
            size={24}
            className={
              variant === "danger"
                ? "icon-danger"
                : variant === "warning"
                ? "icon-warning"
                : ""
            }
          />
        </div>
        <h3>{title}</h3>
        <div className="modal-body">
          <p>{message}</p>
          {detail && <p className="modal-note">{detail}</p>}
        </div>
        <div className="modal-actions">
          <button className="modal-cancel" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            className={`modal-confirm ${variant === "danger" ? "modal-confirm-danger" : ""}`}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
