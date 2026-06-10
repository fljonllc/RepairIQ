import { useState } from "react";
import { Shield, Clock, FolderArchive, Info } from "lucide-react";

interface SettingsProps {
  settings: AppSettings;
  onSave: (settings: AppSettings) => void;
}

export interface AppSettings {
  defaultRetentionDays: number;
  minSizeThresholdMB: number;
  autoScanOnLaunch: boolean;
  showProtectedItems: boolean;
  archiveVerification: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  defaultRetentionDays: 14,
  minSizeThresholdMB: 1,
  autoScanOnLaunch: false,
  showProtectedItems: true,
  archiveVerification: true,
};

export function Settings({ settings, onSave }: SettingsProps) {
  const [local, setLocal] = useState<AppSettings>(settings);

  const update = (key: keyof AppSettings, value: number | boolean) => {
    setLocal((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = () => {
    onSave(local);
  };

  return (
    <div className="settings">
      <div className="settings-header">
        <h2>Settings</h2>
        <p className="settings-subtitle">Configure how RepairIQ behaves</p>
      </div>

      <div className="settings-sections">
        {/* Recovery Vault */}
        <div className="settings-section">
          <div className="settings-section-header">
            <Clock size={18} />
            <h3>Recovery Vault</h3>
          </div>
          <div className="settings-row">
            <div className="settings-row-info">
              <label>Default Retention Period</label>
              <span className="settings-hint">
                How long items stay in the vault before permanent deletion
              </span>
            </div>
            <select
              value={local.defaultRetentionDays}
              onChange={(e) => update("defaultRetentionDays", Number(e.target.value))}
            >
              <option value={7}>7 days</option>
              <option value={14}>14 days</option>
              <option value={30}>30 days</option>
            </select>
          </div>
        </div>

        {/* Scanning */}
        <div className="settings-section">
          <div className="settings-section-header">
            <Shield size={18} />
            <h3>Scanning</h3>
          </div>
          <div className="settings-row">
            <div className="settings-row-info">
              <label>Minimum Size Threshold</label>
              <span className="settings-hint">
                Items smaller than this won't appear in scan results
              </span>
            </div>
            <select
              value={local.minSizeThresholdMB}
              onChange={(e) => update("minSizeThresholdMB", Number(e.target.value))}
            >
              <option value={1}>1 MB</option>
              <option value={5}>5 MB</option>
              <option value={10}>10 MB</option>
              <option value={50}>50 MB</option>
              <option value={100}>100 MB</option>
            </select>
          </div>
          <div className="settings-row">
            <div className="settings-row-info">
              <label>Show Protected Items</label>
              <span className="settings-hint">
                Display system-protected items in the explorer (read-only)
              </span>
            </div>
            <label className="toggle">
              <input
                type="checkbox"
                checked={local.showProtectedItems}
                onChange={(e) => update("showProtectedItems", e.target.checked)}
              />
              <span className="toggle-slider" />
            </label>
          </div>
          <div className="settings-row">
            <div className="settings-row-info">
              <label>Auto-scan on Launch</label>
              <span className="settings-hint">
                Automatically start a storage scan when RepairIQ opens
              </span>
            </div>
            <label className="toggle">
              <input
                type="checkbox"
                checked={local.autoScanOnLaunch}
                onChange={(e) => update("autoScanOnLaunch", e.target.checked)}
              />
              <span className="toggle-slider" />
            </label>
          </div>
        </div>

        {/* Archive */}
        <div className="settings-section">
          <div className="settings-section-header">
            <FolderArchive size={18} />
            <h3>Smart Archive</h3>
          </div>
          <div className="settings-row">
            <div className="settings-row-info">
              <label>Verify Archive Copies</label>
              <span className="settings-hint">
                Compare file sizes after copying to ensure data integrity
              </span>
            </div>
            <label className="toggle">
              <input
                type="checkbox"
                checked={local.archiveVerification}
                onChange={(e) => update("archiveVerification", e.target.checked)}
              />
              <span className="toggle-slider" />
            </label>
          </div>
        </div>

        {/* About */}
        <div className="settings-section">
          <div className="settings-section-header">
            <Info size={18} />
            <h3>About</h3>
          </div>
          <div className="settings-about">
            <p><strong>RepairIQ</strong> v0.1.0</p>
            <p>Diagnose before you act.</p>
            <p className="settings-hint">
              Built with Tauri, React, TypeScript, and Rust.
              <br />
              Your data never leaves your machine.
            </p>
          </div>
        </div>
      </div>

      <div className="settings-footer">
        <button className="settings-save-btn" onClick={handleSave}>
          Save Settings
        </button>
      </div>
    </div>
  );
}
