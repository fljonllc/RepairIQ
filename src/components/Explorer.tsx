import { useState } from "react";
import {
  Shield,
  AlertTriangle,
  Archive,
  Lock,
  FolderOpen,
  ArrowLeft,
  X,
  Zap,
  RotateCcw,
  Info,
  CheckCircle,
} from "lucide-react";
import type { ScanResult, ScannedItem, SafetyLevel } from "../types";
import { formatBytes } from "../utils";

interface ExplorerProps {
  result: ScanResult;
  onMoveToVault: (item: ScannedItem) => void;
  onBatchClean: (items: ScannedItem[]) => void;
  onDrillDown: (path: string) => Promise<void>;
  children: ScannedItem[];
  childrenLoading: boolean;
}

const SafetyIcon = ({ level }: { level: SafetyLevel }) => {
  switch (level) {
    case "Safe":
      return <Shield size={16} className="icon-safe" />;
    case "Review":
      return <AlertTriangle size={16} className="icon-review" />;
    case "Archive":
      return <Archive size={16} className="icon-archive" />;
    case "Protected":
      return <Lock size={16} className="icon-protected" />;
  }
};

function SafetyBadge({ score }: { score: number }) {
  if (score >= 8) return <span className="safety-badge badge-safe">Safe to Remove</span>;
  if (score >= 6) return <span className="safety-badge badge-likely">Likely Safe</span>;
  if (score >= 4) return <span className="safety-badge badge-caution">Use Caution</span>;
  return <span className="safety-badge badge-danger">High Risk</span>;
}

function ConfidenceBadge({ confidence }: { confidence: number }) {
  const colorClass = confidence >= 90 ? "confidence-high"
    : confidence >= 70 ? "confidence-medium"
    : "confidence-low";

  return (
    <span className={`confidence-badge ${colorClass}`}>
      {confidence}%
    </span>
  );
}

function RecommendationBadge({ recommendation, confidence }: { recommendation: string; confidence: number }) {
  const badgeClass = recommendation === "Clean" ? "rec-clean"
    : recommendation === "Archive" ? "rec-archive"
    : recommendation === "Review First" ? "rec-review"
    : "rec-protected";

  const emoji = recommendation === "Clean" ? "🟢"
    : recommendation === "Archive" ? "📦"
    : recommendation === "Review First" ? "⚠️"
    : "🚫";

  return (
    <div className="recommendation-section">
      <span className="recommendation-label">RepairIQ Recommendation</span>
      <div className={`recommendation-badge ${badgeClass}`}>
        <span className="rec-emoji">{emoji}</span>
        <span className="rec-text">{recommendation || "Review First"}</span>
      </div>
      <div className="recommendation-confidence">
        Confidence: <ConfidenceBadge confidence={confidence} />
      </div>
    </div>
  );
}

function ActionButton({ item, onAction }: { item: ScannedItem; onAction: () => void }) {
  if (item.safety === "Protected" || item.recommendation === "Do Not Touch") {
    return (
      <div className="detail-protected-notice">
        <Lock size={14} />
        <span>🚫 Protected — Cannot be removed</span>
      </div>
    );
  }

  const btnClass = item.recommendation === "Clean" ? "action-btn-clean"
    : item.recommendation === "Archive" ? "action-btn-archive"
    : "action-btn-remove";

  const emoji = item.recommendation === "Clean" ? "🟢"
    : item.recommendation === "Archive" ? "📦"
    : "🗑";

  const label = item.action_label || (item.recommendation === "Clean" ? "Clean Cache" : item.recommendation === "Archive" ? "Archive Project" : "Remove");

  return (
    <div className="action-button-section">
      <button className={`detail-action-btn-context ${btnClass}`} onClick={onAction}>
        {emoji} {label}
      </button>
      <span className="estimated-recovery">
        Estimated Recovery: {formatBytes(item.size_bytes)}
      </span>
    </div>
  );
}

export function Explorer({
  result,
  onMoveToVault,
  onBatchClean,
  onDrillDown,
  children,
  childrenLoading,
}: ExplorerProps) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [drillPath, setDrillPath] = useState<string[]>([]);
  const [selectedItem, setSelectedItem] = useState<ScannedItem | null>(null);

  const currentItems =
    drillPath.length > 0
      ? children
      : selectedCategory
      ? result.items.filter((i) => i.category === selectedCategory)
      : result.items;

  // Safe items that can be batch cleaned (exclude apps and paths needing root)
  const safeItems = currentItems.filter(
    (i) =>
      i.safety === "Safe" &&
      i.safety_score >= 8 &&
      i.category !== "Applications" &&
      !i.path.startsWith("/Applications") &&
      i.recommendation === "Clean"
  );
  const safeTotalBytes = safeItems.reduce((sum, i) => sum + i.size_bytes, 0);

  const handleDrill = async (item: ScannedItem) => {
    setDrillPath((prev) => [...prev, item.path]);
    await onDrillDown(item.path);
  };

  const handleBack = async () => {
    const newPath = drillPath.slice(0, -1);
    setDrillPath(newPath);
    if (newPath.length > 0) {
      await onDrillDown(newPath[newPath.length - 1]);
    }
  };

  return (
    <div className="explorer">
      {/* Category Filter Bar */}
      <div className="explorer-filters">
        <button
          className={`filter-btn ${!selectedCategory ? "active" : ""}`}
          onClick={() => {
            setSelectedCategory(null);
            setDrillPath([]);
          }}
        >
          All
        </button>
        {result.categories.map((cat) => (
          <button
            key={cat.name}
            className={`filter-btn ${
              selectedCategory === cat.name ? "active" : ""
            }`}
            onClick={() => {
              setSelectedCategory(cat.name);
              setDrillPath([]);
            }}
          >
            {cat.name}
            <span className="filter-size">{formatBytes(cat.size_bytes)}</span>
          </button>
        ))}
      </div>

      {/* Batch Clean Bar */}
      {safeItems.length > 1 && (
        <div className="batch-clean-bar">
          <div className="batch-clean-info">
            <Shield size={16} className="icon-safe" />
            <span className="batch-clean-count">{safeItems.length} items safe to clean</span>
            <span className="batch-clean-size">{formatBytes(safeTotalBytes)}</span>
          </div>
          <button
            className="batch-clean-btn"
            onClick={() => onBatchClean(safeItems)}
          >
            🟢 Clean All Safe Items → {formatBytes(safeTotalBytes)}
          </button>
        </div>
      )}

      {/* Breadcrumb */}
      {drillPath.length > 0 && (
        <div className="explorer-breadcrumb">
          <button className="back-btn" onClick={handleBack}>
            <ArrowLeft size={16} /> Back
          </button>
          <span className="breadcrumb-path">
            {drillPath[drillPath.length - 1]}
          </span>
        </div>
      )}

      <div className="explorer-main">
        {/* Items List */}
        <div className="explorer-list-container">
          <div className="explorer-list">
            {childrenLoading ? (
              <div className="loading">Scanning directory...</div>
            ) : (
              currentItems.map((item) => (
                <div
                  key={item.path}
                  className={`explorer-item safety-${item.safety.toLowerCase()} ${
                    selectedItem?.path === item.path ? "selected" : ""
                  }`}
                  onClick={() => setSelectedItem(item)}
                >
                  <div className="item-left">
                    <SafetyIcon level={item.safety} />
                    <div className="item-info">
                      <div className="item-name-row">
                        <span className="item-name">{item.name}</span>
                        <SafetyBadge score={item.safety_score} />
                      </div>
                      <span className="item-desc">{item.description}</span>
                      {item.last_accessed_days !== null && (
                        <span className="item-access">
                          Last accessed {item.last_accessed_days} days ago
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="item-right">
                    <span className="item-size">{formatBytes(item.size_bytes)}</span>
                    <div className="item-actions">
                      <button
                        className="drill-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDrill(item);
                        }}
                        title="Explore contents"
                      >
                        <FolderOpen size={14} />
                      </button>
                      {item.safety !== "Protected" && !item.path.startsWith("/Applications") && (
                        <button
                          className="vault-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            onMoveToVault(item);
                          }}
                          title="Move to Recovery Vault"
                        >
                          Move to Vault
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Detail Panel — Redesigned */}
        {selectedItem && (
          <div className="detail-panel">
            <div className="detail-header">
              <h3>Storage Advisor</h3>
              <button
                className="detail-close"
                onClick={() => setSelectedItem(null)}
              >
                <X size={16} />
              </button>
            </div>

            {/* A) RepairIQ Recommendation + Confidence */}
            <RecommendationBadge
              recommendation={selectedItem.recommendation}
              confidence={selectedItem.confidence}
            />

            {/* B) Context-aware action button */}
            <ActionButton item={selectedItem} onAction={() => onMoveToVault(selectedItem)} />

            {/* C) Why RepairIQ Recommends This — Evidence List */}
            {selectedItem.evidence && selectedItem.evidence.length > 0 && (
              <div className="detail-section evidence-section">
                <div className="detail-section-label">
                  <CheckCircle size={13} /> Why RepairIQ Recommends This
                </div>
                <div className="evidence-list">
                  {selectedItem.evidence.map((item, i) => (
                    <div key={i} className={`evidence-item ${item.startsWith("✓") ? "evidence-pass" : "evidence-fail"}`}>
                      <span>{item}</span>
                    </div>
                  ))}
                </div>
                {selectedItem.why_recommended && (
                  <p className="why-recommended-text">{selectedItem.why_recommended}</p>
                )}
              </div>
            )}

            {/* D) Impact Analysis */}
            <div className="detail-section">
              <div className="detail-section-label">
                <Zap size={13} /> Impact Analysis
              </div>
              <div className="impact-analysis">
                <div className="impact-row">
                  <span className="impact-label">You gain:</span>
                  <span className="impact-value impact-gain">{formatBytes(selectedItem.size_bytes)}</span>
                </div>
                <div className="impact-row">
                  <span className="impact-label">You lose:</span>
                  <span className="impact-value">
                    {selectedItem.safety === "Safe" ? "Nothing permanent" : selectedItem.impact}
                  </span>
                </div>
                <div className="impact-row">
                  <span className="impact-label">Side Effects:</span>
                  <span className="impact-value">{selectedItem.side_effects || "None"}</span>
                </div>
                <div className="impact-row">
                  <span className="impact-label">Time To Rebuild:</span>
                  <span className="impact-value">{selectedItem.time_to_rebuild || "N/A"}</span>
                </div>
                <div className="impact-row">
                  <span className="impact-label">Risk:</span>
                  <span className={`impact-value impact-risk-${(selectedItem.risk_level || "medium").toLowerCase()}`}>
                    {selectedItem.risk_level || "Medium"}
                  </span>
                </div>
              </div>
            </div>

            {/* E) What If We're Wrong? */}
            {selectedItem.what_if_wrong && selectedItem.what_if_wrong !== "N/A — RepairIQ will not allow removal of this item." && (
              <div className="detail-section what-if-wrong">
                <div className="detail-section-label">
                  <RotateCcw size={13} /> What If We're Wrong?
                </div>
                <p className="what-if-wrong-text">{selectedItem.what_if_wrong}</p>
              </div>
            )}

            {/* F) Details */}
            <div className="detail-section">
              <div className="detail-section-label">
                <Info size={13} /> Details
              </div>
              <div className="impact-analysis">
                <div className="impact-row">
                  <span className="impact-label">Owner:</span>
                  <span className="impact-value">{selectedItem.owner}</span>
                </div>
                <div className="impact-row">
                  <span className="impact-label">Path:</span>
                  <span className="impact-value detail-path-inline">{selectedItem.path}</span>
                </div>
                {selectedItem.file_count > 0 && (
                  <div className="impact-row">
                    <span className="impact-label">Files:</span>
                    <span className="impact-value">{selectedItem.file_count.toLocaleString()} files</span>
                  </div>
                )}
                {selectedItem.clean_command && (
                  <div className="impact-row">
                    <span className="impact-label">Command:</span>
                    <span className="impact-value">
                      <code className="detail-command-inline">{selectedItem.clean_command}</code>
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Dependencies */}
            {selectedItem.depends_on.length > 0 && (
              <div className="detail-section">
                <div className="detail-section-label">
                  <AlertTriangle size={13} /> Depended on by
                </div>
                <div className="detail-deps">
                  {selectedItem.depends_on.map((dep, i) => (
                    <span key={i} className="dep-tag">{dep}</span>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
