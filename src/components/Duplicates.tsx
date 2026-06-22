import { useState } from "react";
import { Copy } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { DuplicateGroup } from "../types";
import { formatBytes } from "../utils";

export function Duplicates() {
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [scanned, setScanned] = useState(false);

  const scan = async () => {
    setLoading(true);
    try {
      const result = await invoke<DuplicateGroup[]>("detect_duplicates");
      setGroups(result);
      setScanned(true);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const totalWasted = groups.reduce((sum, g) => sum + g.total_wasted, 0);

  return (
    <div className="duplicates-view">
      <div className="duplicates-header">
        <div>
          <h2>Duplicate Files</h2>
          <p className="duplicates-sub">Find identical files wasting space across Desktop, Documents, and Downloads.</p>
        </div>
        <button className="scan-dupes-btn" onClick={scan} disabled={loading}>
          {loading ? "Scanning..." : "Find Duplicates"}
        </button>
      </div>

      {scanned && groups.length === 0 && (
        <div className="duplicates-empty">
          <p>No duplicate files found. Your storage is clean!</p>
        </div>
      )}

      {groups.length > 0 && (
        <>
          <div className="duplicates-stats">
            <span>{groups.length} duplicate groups</span>
            <span className="dupes-wasted">Wasted space: {formatBytes(totalWasted)}</span>
          </div>
          <div className="duplicates-list">
            {groups.map((group) => (
              <div key={group.hash} className="duplicate-group">
                <div className="dupe-header">
                  <Copy size={14} />
                  <span className="dupe-name">{group.file_name}</span>
                  <span className="dupe-count">{group.count} copies</span>
                  <span className="dupe-wasted">{formatBytes(group.total_wasted)} wasted</span>
                </div>
                <div className="dupe-paths">
                  {group.paths.map((path, i) => (
                    <span key={i} className="dupe-path">
                      {i === 0 ? "📁 Keep: " : "🗑 Remove: "}{path}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
