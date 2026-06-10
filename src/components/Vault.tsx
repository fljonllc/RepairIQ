import { RotateCcw, Trash2, Clock } from "lucide-react";
import type { VaultItem } from "../types";
import { formatBytes } from "../utils";

interface VaultProps {
  items: VaultItem[];
  loading: boolean;
  onRestore: (id: number) => void;
  onPurge: () => void;
}

function daysUntilExpiry(expiresAt: string): number {
  const now = new Date();
  const expires = new Date(expiresAt);
  const diff = expires.getTime() - now.getTime();
  return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
}

export function Vault({ items, loading, onRestore, onPurge }: VaultProps) {
  const totalSize = items.reduce((sum, item) => sum + item.size_bytes, 0);

  return (
    <div className="vault">
      <div className="vault-header">
        <div>
          <h2>Recovery Vault</h2>
          <p className="vault-subtitle">
            Nothing is permanently deleted. Items live here until their retention
            period expires.
          </p>
        </div>
        <div className="vault-stats">
          <span className="vault-total">
            {items.length} items · {formatBytes(totalSize)} recoverable
          </span>
          {items.length > 0 && (
            <button className="purge-btn" onClick={onPurge}>
              <Trash2 size={14} /> Purge Expired
            </button>
          )}
        </div>
      </div>

      {loading ? (
        <div className="loading">Loading vault...</div>
      ) : items.length === 0 ? (
        <div className="vault-empty">
          <p>Your vault is empty. Items you remove will appear here for safe recovery.</p>
        </div>
      ) : (
        <div className="vault-list">
          {items.map((item) => {
            const daysLeft = daysUntilExpiry(item.expires_at);
            return (
              <div key={item.id} className="vault-item">
                <div className="vault-item-info">
                  <span className="vault-item-name">{item.name}</span>
                  <span className="vault-item-path">{item.original_path}</span>
                  <span className="vault-item-meta">
                    <Clock size={12} />
                    {daysLeft} days until permanent deletion · {item.category}
                  </span>
                </div>
                <div className="vault-item-right">
                  <span className="vault-item-size">
                    {formatBytes(item.size_bytes)}
                  </span>
                  <button
                    className="restore-btn"
                    onClick={() => onRestore(item.id)}
                  >
                    <RotateCcw size={14} /> Restore
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
