import { useState } from "react";
import { Globe } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { BrowserCache } from "../types";
import { formatBytes } from "../utils";

export function Browsers() {
  const [caches, setCaches] = useState<BrowserCache[]>([]);
  const [loading, setLoading] = useState(false);
  const [scanned, setScanned] = useState(false);

  const scan = async () => {
    setLoading(true);
    try {
      const result = await invoke<BrowserCache[]>("detect_browser_caches");
      setCaches(result);
      setScanned(true);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const totalSize = caches.reduce((sum, c) => sum + c.size_bytes, 0);

  return (
    <div className="browsers-view">
      <div className="browsers-header">
        <div>
          <h2>Browser Caches</h2>
          <p className="browsers-sub">Detect cached data from all your browsers.</p>
        </div>
        <button className="scan-browsers-btn" onClick={scan} disabled={loading}>
          {loading ? "Detecting..." : "Detect Caches"}
        </button>
      </div>

      {scanned && caches.length === 0 && (
        <div className="browsers-empty">
          <p>No significant browser caches detected.</p>
        </div>
      )}

      {caches.length > 0 && (
        <>
          <div className="browsers-stats">
            Total browser cache: {formatBytes(totalSize)}
          </div>
          <div className="browsers-list">
            {caches.map((cache) => (
              <div key={cache.path} className="browser-item">
                <div className="browser-info">
                  <Globe size={16} />
                  <span className="browser-name">{cache.browser}</span>
                </div>
                <span className="browser-size">{formatBytes(cache.size_bytes)}</span>
                <code className="browser-command">{cache.clean_command}</code>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
