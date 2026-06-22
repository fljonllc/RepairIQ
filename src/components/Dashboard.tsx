import { useState, useEffect, useRef } from "react";
import {
  Shield,
  AlertTriangle,
  Archive,
  HardDrive,
  Clock,
  Zap,
} from "lucide-react";
import type { ScanResult, ScannedItem } from "../types";
import { formatBytes } from "../utils";

interface DashboardProps {
  result: ScanResult;
  onShowWhy: () => void;
  onMoveToVault?: (item: ScannedItem) => void;
  onQuickClean?: (items: ScannedItem[]) => void;
}

const CATEGORY_COLORS = [
  "#ef4444",
  "#f59e0b",
  "#10b981",
  "#3b82f6",
  "#8b5cf6",
  "#ec4899",
  "#14b8a6",
  "#f97316",
];

const CATEGORY_NAMES: Record<string, string> = {
  "System Data": "System & app caches",
  Applications: "Applications you've installed",
  Docker: "Docker containers & images",
  Developer: "Developer tool caches",
  "Virtual Machines": "Virtual machines",
  Messages: "Messages & attachments",
  Music: "Music & audio",
  Downloads: "Downloaded files",
  Documents: "Your documents",
  Desktop: "Desktop files",
  Trash: "Trash (ready to empty)",
};

function getRiskLabel(confidence: number): string {
  if (confidence >= 95) return "None";
  if (confidence >= 85) return "Very Low";
  if (confidence >= 70) return "Low";
  if (confidence >= 50) return "Medium";
  return "High";
}

export function Dashboard({ result, onShowWhy, onMoveToVault, onQuickClean }: DashboardProps) {
  const totalRecovery =
    result.safe_recovery_bytes +
    result.review_recovery_bytes +
    result.archive_recovery_bytes;

  // Track previous free_bytes to show "+X freed" indicator
  const prevFreeRef = useRef(result.free_bytes);
  const [freedAmount, setFreedAmount] = useState<number | null>(null);

  useEffect(() => {
    const diff = result.free_bytes - prevFreeRef.current;
    if (diff > 0) {
      setFreedAmount(diff);
      const timer = setTimeout(() => setFreedAmount(null), 3000);
      prevFreeRef.current = result.free_bytes;
      return () => clearTimeout(timer);
    }
    prevFreeRef.current = result.free_bytes;
  }, [result.free_bytes]);

  // Quick Clean items: confidence >= 95, not Applications, not /Applications, recommendation === "Clean"
  const quickCleanItems = result.items.filter(
    (item) =>
      item.confidence >= 95 &&
      item.category !== "Applications" &&
      !item.path.startsWith("/Applications") &&
      item.recommendation === "Clean"
  );

  const quickCleanBytes = quickCleanItems.reduce((sum, item) => sum + item.size_bytes, 0);

  // Top 3 Opportunities
  const topOpportunities = result.items
    .filter(
      (item) =>
        item.safety === "Safe" &&
        item.confidence >= 80 &&
        item.category !== "Applications" &&
        item.category !== "Documents" &&
        item.category !== "Desktop" &&
        !item.path.startsWith("/Applications") &&
        (item.recommendation === "Clean" || item.action_label === "Clean Cache")
    )
    .sort((a, b) => {
      const confDiff = b.confidence - a.confidence;
      if (Math.abs(confDiff) > 5) return confDiff;
      return b.size_bytes - a.size_bytes;
    })
    .slice(0, 3);

  const totalOpportunityBytes = topOpportunities.reduce((sum, item) => sum + item.size_bytes, 0);

  const handleFixAll = () => {
    if (onMoveToVault && topOpportunities.length > 0) {
      topOpportunities.forEach((item) => onMoveToVault(item));
    }
  };

  const handleQuickClean = () => {
    if (onQuickClean && quickCleanItems.length > 0) {
      onQuickClean(quickCleanItems);
    }
  };

  // Space breakdown: categories sorted by size descending
  const sortedCategories = [...result.categories].sort(
    (a, b) => b.size_bytes - a.size_bytes
  );

  // Total cleanable bytes (items with recommendation "Clean" and safety "Safe")
  const totalCleanable = result.items
    .filter((item) => item.safety === "Safe" && item.recommendation === "Clean")
    .reduce((sum, item) => sum + item.size_bytes, 0);

  return (
    <div className="dashboard">
      {/* 0. Quick Clean Button — Top of dashboard */}
      {quickCleanItems.length > 0 && (
        <button className="quick-clean-btn" onClick={handleQuickClean}>
          <div>
            <span>⚡ Quick Clean — {formatBytes(quickCleanBytes)}</span>
            <div className="quick-clean-sub">
              Items scored 95%+ confidence. Zero risk.
            </div>
          </div>
        </button>
      )}

      {/* 1. Today's Opportunities */}
      {topOpportunities.length > 0 && (
        <div className="opportunities-section">
          <h3 className="opportunities-header">
            <Zap size={18} /> Today's Opportunities
          </h3>
          <div className="opportunities-list-new">
            {topOpportunities.map((item, index) => (
              <div key={item.path} className="opportunity-row">
                <span className="opportunity-number">{index + 1}.</span>
                <div className="opportunity-details">
                  <span className="opportunity-name">{item.name}</span>
                  <div className="opportunity-meta">
                    <span className="opportunity-recover">
                      Recover: {formatBytes(item.size_bytes)}
                    </span>
                    <span className="opportunity-meta-sep">|</span>
                    <span className="opportunity-risk">
                      Risk: {getRiskLabel(item.confidence)}
                    </span>
                    <span className="opportunity-meta-sep">|</span>
                    <span className="opportunity-confidence">
                      {item.confidence}%
                    </span>
                  </div>
                </div>
                {onMoveToVault && (
                  <button
                    className="opportunity-action-btn"
                    onClick={() => onMoveToVault(item)}
                  >
                    Clean Now
                  </button>
                )}
              </div>
            ))}
          </div>
          {onMoveToVault && topOpportunities.length > 1 && (
            <button className="fix-all-btn" onClick={handleFixAll}>
              Fix All {topOpportunities.length} → Total: {formatBytes(totalOpportunityBytes)}
            </button>
          )}
        </div>
      )}

      {/* 2. Storage Overview (animated bar) */}
      <div className="hero-card">
        <div className="hero-header">
          <HardDrive size={24} />
          <h2>Storage Overview</h2>
          <span className="scan-time">
            <Clock size={14} />
            Scanned in {(result.scan_duration_ms / 1000).toFixed(1)}s
          </span>
        </div>

        <div className="storage-bar">
          <div
            className="storage-used"
            style={{
              width: `${(result.used_bytes / result.total_bytes) * 100}%`,
            }}
          />
        </div>
        <div className="storage-labels">
          <span>Used: {formatBytes(result.used_bytes)}</span>
          <span>
            Free: {formatBytes(result.free_bytes)}
            {freedAmount && (
              <span className="freed-indicator"> +{formatBytes(freedAmount)} freed</span>
            )}
          </span>
          <span>Total: {formatBytes(result.total_bytes)}</span>
        </div>
      </div>

      {/* 3. Recovery Summary (compact grid, no title) */}
      <div className="recovery-card">
        <div className="recovery-grid">
          <div className="recovery-item safe">
            <Shield size={16} />
            <span className="recovery-label">Safe</span>
            <span className="recovery-value">
              {formatBytes(result.safe_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item review">
            <AlertTriangle size={16} />
            <span className="recovery-label">Review</span>
            <span className="recovery-value">
              {formatBytes(result.review_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item archive">
            <Archive size={16} />
            <span className="recovery-label">Archive</span>
            <span className="recovery-value">
              {formatBytes(result.archive_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item total">
            <HardDrive size={16} />
            <span className="recovery-label">Total</span>
            <span className="recovery-value">{formatBytes(totalRecovery)}</span>
          </div>
        </div>
      </div>

      {/* 4. Where's Your Space Going? — Plain language breakdown */}
      <div className="story-card">
        <h3 style={{ fontSize: "13px", marginBottom: "8px" }}>Where's your space going?</h3>
        <div className="space-breakdown">
          {sortedCategories.map((cat, i) => (
            <div key={cat.name} className="space-row">
              <span
                className="story-dot"
                style={{
                  background: CATEGORY_COLORS[i % CATEGORY_COLORS.length],
                }}
              />
              <span className="space-row-name">
                {CATEGORY_NAMES[cat.name] || cat.name}
              </span>
              <span className="space-row-size">{formatBytes(cat.size_bytes)}</span>
            </div>
          ))}
        </div>
        {totalCleanable > 0 && (
          <div className="space-cleanable" onClick={onShowWhy}>
            <span className="space-cleanable-label">Things you can clean today</span>
            <span className="space-cleanable-size">{formatBytes(totalCleanable)}</span>
          </div>
        )}
      </div>
    </div>
  );
}
