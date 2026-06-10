import { useState } from "react";
import {
  HardDrive,
  ExternalLink,
  CheckCircle,
  Clock,
  FolderArchive,
  AlertCircle,
} from "lucide-react";
import type { ArchiveRecommendation } from "../types";
import { formatBytes } from "../utils";

interface SmartArchiveProps {
  candidates: ArchiveRecommendation[];
  volumes: string[];
  loading: boolean;
  archiving: boolean;
  onScan: () => void;
  onLoadVolumes: () => void;
  onArchive: (sourcePath: string, destinationDir: string) => Promise<string>;
  onToast: (type: "success" | "error" | "info", message: string) => void;
}

export function SmartArchive({
  candidates,
  volumes,
  loading,
  archiving,
  onScan,
  onLoadVolumes,
  onArchive,
  onToast,
}: SmartArchiveProps) {
  const [selectedVolume, setSelectedVolume] = useState<string>("");
  const [confirmItem, setConfirmItem] = useState<ArchiveRecommendation | null>(null);
  const [archivedItems, setArchivedItems] = useState<Set<string>>(new Set());

  const totalArchivable = candidates.reduce((sum, c) => sum + c.size_bytes, 0);

  const handleArchive = async (item: ArchiveRecommendation) => {
    if (!selectedVolume) {
      onToast("error", "Please select a destination volume first");
      return;
    }

    try {
      const destPath = await onArchive(item.path, selectedVolume);
      setArchivedItems((prev) => new Set([...prev, item.path]));
      setConfirmItem(null);
      onToast(
        "success",
        `Archived "${item.name}" to ${destPath}. Copy verified. Original is safe to remove.`
      );
    } catch (e) {
      onToast("error", `Archive failed: ${String(e)}`);
    }
  };

  return (
    <div className="smart-archive">
      <div className="archive-header">
        <div>
          <h2>Smart Archive</h2>
          <p className="archive-subtitle">
            Old projects detected. Archive to external storage, verified before
            suggesting removal.
          </p>
        </div>
        <div className="archive-actions">
          <button className="scan-archive-btn" onClick={() => { onScan(); onLoadVolumes(); }}>
            <FolderArchive size={14} /> Scan for Projects
          </button>
        </div>
      </div>

      {/* Volume Selector */}
      {candidates.length > 0 && (
        <div className="volume-selector">
          <label>
            <HardDrive size={14} />
            Archive Destination:
          </label>
          <select
            value={selectedVolume}
            onChange={(e) => setSelectedVolume(e.target.value)}
          >
            <option value="">Select external drive...</option>
            {volumes.map((vol) => (
              <option key={vol} value={vol}>
                {vol}
              </option>
            ))}
          </select>
          {volumes.length === 0 && (
            <span className="no-volumes">
              <AlertCircle size={12} /> No external drives detected
            </span>
          )}
        </div>
      )}

      {/* Stats */}
      {candidates.length > 0 && (
        <div className="archive-stats">
          <span>{candidates.length} projects found</span>
          <span className="archive-stats-sep">·</span>
          <span>{formatBytes(totalArchivable)} total</span>
        </div>
      )}

      {loading ? (
        <div className="loading">Scanning for old projects...</div>
      ) : candidates.length === 0 ? (
        <div className="archive-empty">
          <FolderArchive size={40} strokeWidth={1.5} />
          <p>Click "Scan for Projects" to find old projects that could be archived.</p>
          <p className="archive-empty-sub">
            We'll look in Developer, Projects, Desktop, and Documents folders
            for projects not opened in 90+ days.
          </p>
        </div>
      ) : (
        <div className="archive-list">
          {candidates.map((item) => {
            const isArchived = archivedItems.has(item.path);
            return (
              <div
                key={item.path}
                className={`archive-item ${isArchived ? "archived" : ""}`}
              >
                <div className="archive-item-info">
                  <div className="archive-item-header">
                    <span className="archive-item-name">{item.name}</span>
                    <span className="archive-item-type">{item.project_type}</span>
                  </div>
                  <span className="archive-item-path">{item.path}</span>
                  <div className="archive-item-meta">
                    <span className="archive-meta-tag">
                      <Clock size={11} />
                      Last opened {item.last_opened_days} days ago
                    </span>
                    <span className={`archive-status ${item.status.includes("Strong") ? "strong" : item.status.includes("Recommended") ? "recommended" : "consider"}`}>
                      {item.status}
                    </span>
                  </div>
                </div>
                <div className="archive-item-right">
                  <span className="archive-item-size">
                    {formatBytes(item.size_bytes)}
                  </span>
                  {isArchived ? (
                    <span className="archive-done">
                      <CheckCircle size={14} /> Archived
                    </span>
                  ) : (
                    <button
                      className="archive-btn"
                      onClick={() => setConfirmItem(item)}
                      disabled={archiving || !selectedVolume}
                    >
                      <ExternalLink size={14} />
                      Move to External Drive
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Confirmation Modal */}
      {confirmItem && (
        <div className="modal-overlay" onClick={() => setConfirmItem(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Archive Project</h3>
            <div className="modal-body">
              <p>
                <strong>{confirmItem.name}</strong> ({formatBytes(confirmItem.size_bytes)})
              </p>
              <p>Will be copied to: <code>{selectedVolume}/{confirmItem.name}</code></p>
              <p className="modal-note">
                The copy will be verified before anything changes. Your original
                project stays untouched until you manually delete it.
              </p>
            </div>
            <div className="modal-actions">
              <button className="modal-cancel" onClick={() => setConfirmItem(null)}>
                Cancel
              </button>
              <button
                className="modal-confirm"
                onClick={() => handleArchive(confirmItem)}
                disabled={archiving}
              >
                {archiving ? "Copying & Verifying..." : "Archive Now"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
