import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
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

function getRiskLabel(confidence: number): string {
  if (confidence >= 95) return "None";
  if (confidence >= 85) return "Very Low";
  if (confidence >= 70) return "Low";
  if (confidence >= 50) return "Medium";
  return "High";
}

export function Dashboard({ result, onShowWhy, onMoveToVault }: DashboardProps) {
  const pieData = result.categories.map((cat) => ({
    name: cat.name,
    value: cat.size_bytes,
  }));

  const totalRecovery =
    result.safe_recovery_bytes +
    result.review_recovery_bytes +
    result.archive_recovery_bytes;

  // Top 3 Opportunities: highest confidence + biggest size (safe items)
  const topOpportunities = result.items
    .filter((item) => item.safety === "Safe" && item.confidence >= 75)
    .sort((a, b) => {
      // Sort by confidence first, then by size
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

  return (
    <div className="dashboard">
      {/* 1. Today's Opportunities — FIRST, most prominent */}
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

      {/* 2. Storage Overview (compact bar) */}
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
          <span>Free: {formatBytes(result.free_bytes)}</span>
          <span>Total: {formatBytes(result.total_bytes)}</span>
        </div>
      </div>

      {/* 3. Recovery Summary (compact grid) */}
      <div className="recovery-card">
        <h3>Recovery Potential</h3>
        <div className="recovery-grid">
          <div className="recovery-item safe">
            <Shield size={20} />
            <span className="recovery-label">Safe Recovery</span>
            <span className="recovery-value">
              {formatBytes(result.safe_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item review">
            <AlertTriangle size={20} />
            <span className="recovery-label">Review Recovery</span>
            <span className="recovery-value">
              {formatBytes(result.review_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item archive">
            <Archive size={20} />
            <span className="recovery-label">Archive Recovery</span>
            <span className="recovery-value">
              {formatBytes(result.archive_recovery_bytes)}
            </span>
          </div>
          <div className="recovery-item total">
            <HardDrive size={20} />
            <span className="recovery-label">Total Potential</span>
            <span className="recovery-value">{formatBytes(totalRecovery)}</span>
          </div>
        </div>

        <button className="show-why-btn" onClick={onShowWhy}>
          Show Me Why →
        </button>
      </div>

      {/* 4. Storage Story — Category Breakdown */}
      <div className="story-card">
        <h3>Storage Story</h3>
        <div className="story-layout">
          <div className="story-chart">
            <ResponsiveContainer width="100%" height={180}>
              <PieChart>
                <Pie
                  data={pieData}
                  cx="50%"
                  cy="50%"
                  innerRadius={45}
                  outerRadius={75}
                  dataKey="value"
                  paddingAngle={2}
                >
                  {pieData.map((_, index) => (
                    <Cell
                      key={`cell-${index}`}
                      fill={CATEGORY_COLORS[index % CATEGORY_COLORS.length]}
                    />
                  ))}
                </Pie>
                <Tooltip
                  formatter={(value) => formatBytes(Number(value))}
                  contentStyle={{
                    background: "#1e1e2e",
                    border: "1px solid #333",
                    borderRadius: "8px",
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>
          <div className="story-breakdown">
            {result.categories.map((cat, i) => (
              <div key={cat.name} className="story-row">
                <span
                  className="story-dot"
                  style={{
                    background: CATEGORY_COLORS[i % CATEGORY_COLORS.length],
                  }}
                />
                <span className="story-name">{cat.name}</span>
                <span className="story-size">{formatBytes(cat.size_bytes)}</span>
                <span className="story-count">{cat.items.length} items</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
