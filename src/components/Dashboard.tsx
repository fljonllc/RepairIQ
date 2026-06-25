import { useState, useEffect, useRef } from "react";
import {
  Shield,
  AlertTriangle,
  Archive,
  HardDrive,
  Clock,
  Zap,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
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
  Mail: "Email & attachments",
  Music: "Music & audio",
  Movies: "Videos & screen recordings",
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

function getGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 17) return "Good afternoon";
  return "Good evening";
}

function getDeviceName(): string {
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("mac")) return "Mac";
  if (platform.includes("win")) return "PC";
  return "computer";
}

function getHealthNarrative(result: ScanResult): string {
  const freePct = Math.round((result.free_bytes / result.total_bytes) * 100);
  const cleanable = formatBytes(result.safe_recovery_bytes);

  if (result.health_grade === "A+" || result.health_grade === "A") {
    if (result.safe_recovery_bytes > 1_000_000_000) {
      return `Your ${getDeviceName()} has enough free space for normal operation. There's ${cleanable} of safe-to-clean data that accumulated from developer tools and caches. Cleaning it will improve performance and free space for future projects.`;
    }
    return `Your ${getDeviceName()} is in great shape. Storage is well-managed with minimal cleanable data. No action needed right now.`;
  }
  if (result.health_grade === "B") {
    return `Your ${getDeviceName()} is healthy but you've dropped below the recommended 20% free storage (currently ${freePct}%). Cleaning today's safe items will create room for future work.`;
  }
  if (result.health_grade === "C") {
    return `Your ${getDeviceName()} is running low on space (${freePct}% free). Developer caches and old data are accumulating. We recommend cleaning safe items today to prevent performance issues.`;
  }
  return `Your ${getDeviceName()} is critically low on space (${freePct}% free). This may cause slowdowns, failed updates, and app crashes. Immediate cleanup is recommended.`;
}

function generateExplanation(result: ScanResult): string {
  const categories = [...result.categories].sort((a, b) => b.size_bytes - a.size_bytes);
  const top1 = categories[0];
  const top2 = categories[1];
  const cleanable = formatBytes(result.safe_recovery_bytes);
  const total = formatBytes(result.used_bytes);

  return `Your ${getDeviceName()} is using ${total} of storage. The largest contributor is ${CATEGORY_NAMES[top1?.name] || top1?.name} (${formatBytes(top1?.size_bytes || 0)}), followed by ${CATEGORY_NAMES[top2?.name] || top2?.name} (${formatBytes(top2?.size_bytes || 0)}). The good news is that ${cleanable} appears immediately recoverable without affecting your work. Most remaining storage belongs to active software and personal files that should be reviewed rather than removed. If this were my computer, I'd start by cleaning the developer caches — they rebuild automatically and free the most space with zero risk.`;
}

export function Dashboard({ result, onShowWhy, onMoveToVault, onQuickClean }: DashboardProps) {
  const totalRecovery =
    result.safe_recovery_bytes +
    result.review_recovery_bytes +
    result.archive_recovery_bytes;

  // Track previous free_bytes to show "+X freed" indicator
  const prevFreeRef = useRef(result.free_bytes);
  const [freedAmount, setFreedAmount] = useState<number | null>(null);
  const [showExplain, setShowExplain] = useState(false);
  const [forecast, setForecast] = useState<{ days_until_full: number | null; daily_growth_bytes: number; weekly_growth_bytes: number; total_cleaned_bytes: number; total_cleaned_count: number } | null>(null);

  // Load forecast on mount + record snapshot
  useEffect(() => {
    const loadForecast = async () => {
      try {
        // Record current state
        await invoke("record_storage_snapshot", { usedBytes: result.used_bytes, freeBytes: result.free_bytes, totalBytes: result.total_bytes });
        // Get forecast
        const fc = await invoke<{ days_until_full: number | null; daily_growth_bytes: number; weekly_growth_bytes: number; total_cleaned_bytes: number; total_cleaned_count: number }>("get_storage_forecast");
        setForecast(fc);
      } catch (e) { console.error(e); }
    };
    loadForecast();
  }, [result.used_bytes]);

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
      {/* Personalized Greeting */}
      <div className="greeting-section">
        <h2 className="greeting-text">
          {getGreeting()}, your ${getDeviceName()} is {result.health_grade === "A" || result.health_grade === "A+" ? "healthy" : result.health_grade === "B" ? "in good shape" : "needs attention"}.
        </h2>
        <p className="greeting-summary">
          {quickCleanItems.length > 0
            ? `You have ${formatBytes(quickCleanBytes)} that can be safely reclaimed in under ${quickCleanItems.length > 5 ? "2 minutes" : "30 seconds"}. No risky cleanups detected.`
            : "Your storage is well-maintained. No immediate action needed."
          }
        </p>
      </div>

      {/* Recommendation Banner */}
      {quickCleanItems.length > 0 && (
        <div className="recommendation-banner">
          <div className="recommendation-banner-header">
            <span className="rec-badge-green">🟢 Recommended</span>
            <span className="rec-confidence">Confidence: 99%</span>
          </div>
          <div className="recommendation-banner-body">
            <span className="rec-main-text">Safely reclaim {formatBytes(quickCleanBytes)}</span>
            <span className="rec-details">
              {quickCleanItems.length} items · Estimated time: {quickCleanItems.length > 5 ? "~2 minutes" : "~30 seconds"} · No restart required
            </span>
          </div>
          <button className="rec-action-btn" onClick={handleQuickClean}>
            Apply Recommendations
          </button>
        </div>
      )}

      {/* Health Score */}
      <div className="health-score-card">
        <div className="health-score-circle">
          <span className={`health-grade grade-${result.health_grade.replace('+', 'plus').toLowerCase()}`}>
            {result.health_grade}
          </span>
        </div>
        <div className="health-score-info">
          <span className="health-score-title">Storage Health</span>
          <p className="health-narrative">
            {getHealthNarrative(result)}
          </p>
        </div>
      </div>

      {/* Today's Opportunities */}
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
                  <span className="opportunity-name">
                    {item.subcategory && item.subcategory !== item.name
                      ? `${item.name} (${item.subcategory})`
                      : item.owner && item.owner !== "You" && item.owner !== "You (Downloads)"
                      ? `${item.name} — ${item.owner}`
                      : item.name}
                  </span>
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
                    Apply
                  </button>
                )}
              </div>
            ))}
          </div>
          {onMoveToVault && topOpportunities.length > 1 && (
            <button className="fix-all-btn" onClick={handleFixAll}>
              Apply {topOpportunities.length} Recommendations → {formatBytes(totalOpportunityBytes)}
            </button>
          )}
        </div>
      )}

      {/* Storage Overview (animated bar) */}
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

      {/* Storage Timeline */}
      {forecast && (forecast.daily_growth_bytes !== 0 || forecast.total_cleaned_bytes > 0) && (
        <div className="timeline-card">
          <h3 className="timeline-title">📈 Your ${getDeviceName()} Over Time</h3>
          <div className="timeline-stats">
            {forecast.daily_growth_bytes > 0 && (
              <span className="timeline-stat timeline-growing">↑ Growing {formatBytes(Math.abs(forecast.weekly_growth_bytes))}/week</span>
            )}
            {forecast.daily_growth_bytes < 0 && (
              <span className="timeline-stat timeline-shrinking">↓ Shrinking {formatBytes(Math.abs(forecast.weekly_growth_bytes))}/week</span>
            )}
            {forecast.days_until_full && forecast.days_until_full < 365 && (
              <span className="timeline-stat timeline-warning">⚠️ Full in ~{forecast.days_until_full} days at this rate</span>
            )}
            {forecast.total_cleaned_bytes > 0 && (
              <span className="timeline-stat timeline-cleaned">✨ You've cleaned {formatBytes(forecast.total_cleaned_bytes)} total ({forecast.total_cleaned_count} times)</span>
            )}
          </div>
        </div>
      )}

      {/* Recovery Summary (compact grid) */}
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

      {/* Where's Your Space Going? */}
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

      {/* Explain My Mac */}
      <div className="explain-section">
        <button className="explain-btn" onClick={() => setShowExplain(!showExplain)}>
          🧠 Explain My Mac
        </button>
        {showExplain && (
          <div className="explain-content">
            <p>{generateExplanation(result)}</p>
          </div>
        )}
      </div>
    </div>
  );
}
