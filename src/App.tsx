import { useEffect, useState, useCallback } from "react";
import {
  Search,
  LayoutDashboard,
  FolderTree,
  Archive,
  FolderArchive,
  Settings as SettingsIcon,
  RefreshCw,
  Copy,
  Globe,
  Activity,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Dashboard } from "./components/Dashboard";
import { Explorer } from "./components/Explorer";
import { Vault } from "./components/Vault";
import { SmartArchive } from "./components/SmartArchive";
import { Duplicates } from "./components/Duplicates";
import { Browsers } from "./components/Browsers";
import { MacHealth } from "./components/MacHealth";
import { Onboarding } from "./components/Onboarding";
import { Settings, DEFAULT_SETTINGS } from "./components/Settings";
import type { AppSettings } from "./components/Settings";
import { Treemap } from "./components/Treemap";
import { ToastContainer } from "./components/Toast";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { useScanner, useVault, useExplorer, useArchive } from "./hooks/useScanner";
import type { View, ScannedItem, Toast } from "./types";
import { formatBytes } from "./utils";
import "./App.css";

function App() {
  const [view, setView] = useState<View>("dashboard");
  const [showTreemap, setShowTreemap] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [confirmAction, setConfirmAction] = useState<{
    item: ScannedItem;
    retentionDays: number;
  } | null>(null);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [hasOnboarded, setHasOnboarded] = useState(() => {
    return localStorage.getItem("repairiq-onboarded") === "true";
  });

  const { scanResult, scanning, error, scan, removeItemFromResults } = useScanner();
  const {
    vaultItems,
    loading: vaultLoading,
    initVault,
    moveToVault,
    restoreFromVault,
    listVault,
    purgeExpired,
  } = useVault();
  const { children, loading: childrenLoading, drillDown } = useExplorer();
  const {
    candidates,
    volumes,
    loading: archiveLoading,
    archiving,
    findCandidates,
    loadVolumes,
    archiveProject,
  } = useArchive();

  // Initialize vault on mount
  useEffect(() => {
    initVault();
    // Load settings from localStorage
    const saved = localStorage.getItem("repairiq-settings");
    if (saved) {
      try {
        setSettings(JSON.parse(saved));
      } catch {
        // Ignore parse errors
      }
    }
  }, [initVault]);

  // Listen for tray "scan" event
  useEffect(() => {
    const unlisten = listen("trigger-scan", () => {
      scan();
    });
    return () => { unlisten.then(fn => fn()); };
  }, [scan]);

  const completeOnboarding = useCallback(() => {
    localStorage.setItem("repairiq-onboarded", "true");
    setHasOnboarded(true);
    scan();
  }, [scan]);

  // Auto-scan on launch if enabled
  useEffect(() => {
    if (settings.autoScanOnLaunch && !scanResult && !scanning) {
      scan();
    }
  }, [settings.autoScanOnLaunch]); // eslint-disable-line react-hooks/exhaustive-deps

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey) {
        switch (e.key) {
          case "1":
            e.preventDefault();
            setView("dashboard");
            break;
          case "2":
            e.preventDefault();
            if (scanResult) setView("explorer");
            break;
          case "3":
            e.preventDefault();
            setView("vault");
            listVault();
            break;
          case "4":
            e.preventDefault();
            setView("archive");
            break;
          case "r":
            e.preventDefault();
            scan();
            break;
          case ",":
            e.preventDefault();
            setView("settings");
            break;
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [scanResult, scan, listVault]);

  // Toast helpers
  const addToast = useCallback((type: Toast["type"], message: string) => {
    const id = Date.now().toString() + Math.random().toString(36).slice(2);
    setToasts((prev) => [...prev, { id, type, message }]);
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // Vault action with confirmation
  const requestMoveToVault = useCallback(
    (item: ScannedItem) => {
      setConfirmAction({ item, retentionDays: settings.defaultRetentionDays });
    },
    [settings.defaultRetentionDays]
  );

  const confirmMoveToVault = useCallback(async () => {
    if (!confirmAction) return;
    const { item, retentionDays } = confirmAction;
    setConfirmAction(null);

    try {
      await moveToVault(item.path, retentionDays, item.category);
      removeItemFromResults(item.path);
      addToast(
        "success",
        `Moved "${item.name}" to vault (${formatBytes(item.size_bytes)} recoverable for ${retentionDays} days)`
      );
    } catch (e) {
      addToast("error", `Failed to move "${item.name}": ${String(e)}`);
    }
  }, [confirmAction, moveToVault, removeItemFromResults, addToast]);

  // Batch clean — move all safe items to vault at once
  const [batchItems, setBatchItems] = useState<ScannedItem[] | null>(null);

  const handleBatchClean = useCallback(
    (items: ScannedItem[]) => {
      setBatchItems(items);
    },
    []
  );

  const confirmBatchClean = useCallback(async () => {
    if (!batchItems) return;
    const items = batchItems;
    setBatchItems(null);

    let cleaned = 0;
    let totalFreed = 0;

    for (const item of items) {
      try {
        await moveToVault(item.path, settings.defaultRetentionDays, item.category);
        removeItemFromResults(item.path);
        cleaned++;
        totalFreed += item.size_bytes;
      } catch (e) {
        addToast("error", `Failed: ${item.name} — ${String(e)}`);
      }
    }

    if (cleaned > 0) {
      addToast(
        "success",
        `Cleaned ${cleaned} items → ${formatBytes(totalFreed)} moved to Recovery Vault`
      );
    }
  }, [batchItems, moveToVault, settings.defaultRetentionDays, removeItemFromResults, addToast]);

  const handleRestore = useCallback(
    async (id: number) => {
      try {
        await restoreFromVault(id);
        addToast("success", "Item restored to original location");
      } catch (e) {
        addToast("error", `Restore failed: ${String(e)}`);
      }
    },
    [restoreFromVault, addToast]
  );

  const handlePurge = useCallback(async () => {
    try {
      const freed = await purgeExpired();
      addToast("info", `Purged expired items, freed ${formatBytes(freed)}`);
    } catch (e) {
      addToast("error", `Purge failed: ${String(e)}`);
    }
  }, [purgeExpired, addToast]);

  const handlePurgeAll = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const freed = await invoke<number>("vault_purge_all");
      addToast("success", `Vault emptied — ${formatBytes(freed)} permanently freed`);
      await listVault();
    } catch (e) {
      addToast("error", `Failed to empty vault: ${String(e)}`);
    }
  }, [addToast, listVault]);

  const handleDeletePermanently = useCallback(async (id: number) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const freed = await invoke<number>("vault_delete_permanently", { id });
      addToast("success", `Permanently deleted — ${formatBytes(freed)} freed`);
      await listVault();
    } catch (e) {
      addToast("error", `Delete failed: ${String(e)}`);
    }
  }, [addToast, listVault]);

  const handleSaveSettings = useCallback(
    (newSettings: AppSettings) => {
      setSettings(newSettings);
      localStorage.setItem("repairiq-settings", JSON.stringify(newSettings));
      addToast("success", "Settings saved");
    },
    [addToast]
  );

  const handleArchive = useCallback(
    async (sourcePath: string, destinationDir: string) => {
      return await archiveProject(sourcePath, destinationDir);
    },
    [archiveProject]
  );

  return (
    <div className="app">
      {/* Sidebar */}
      <nav className="sidebar">
        <div className="sidebar-brand">
          <h1>RepairIQ</h1>
          <span className="brand-sub">Diagnose Before You Act</span>
        </div>
        <div className="sidebar-nav">
          <button
            className={`nav-btn ${view === "dashboard" ? "active" : ""}`}
            onClick={() => setView("dashboard")}
          >
            <LayoutDashboard size={18} /> Dashboard
            <kbd className="nav-shortcut">⌘1</kbd>
          </button>
          <button
            className={`nav-btn ${view === "explorer" ? "active" : ""}`}
            onClick={() => setView("explorer")}
            disabled={!scanResult}
          >
            <FolderTree size={18} /> Explorer
            <kbd className="nav-shortcut">⌘2</kbd>
          </button>
          <button
            className={`nav-btn ${view === "vault" ? "active" : ""}`}
            onClick={() => {
              setView("vault");
              listVault();
            }}
          >
            <Archive size={18} /> Recovery Vault
            <kbd className="nav-shortcut">⌘3</kbd>
          </button>
          <button
            className={`nav-btn ${view === "archive" ? "active" : ""}`}
            onClick={() => setView("archive")}
          >
            <FolderArchive size={18} /> Smart Archive
            <kbd className="nav-shortcut">⌘4</kbd>
          </button>
          <button
            className={`nav-btn ${view === "duplicates" ? "active" : ""}`}
            onClick={() => setView("duplicates")}
          >
            <Copy size={18} /> Duplicates
          </button>
          <button
            className={`nav-btn ${view === "browsers" ? "active" : ""}`}
            onClick={() => setView("browsers")}
          >
            <Globe size={18} /> Browsers
          </button>
          <button
            className={`nav-btn ${view === "health" ? "active" : ""}`}
            onClick={() => setView("health")}
          >
            <Activity size={18} /> Mac Health
          </button>

          <div className="nav-divider" />

          <button
            className={`nav-btn ${view === "settings" ? "active" : ""}`}
            onClick={() => setView("settings")}
          >
            <SettingsIcon size={18} /> Settings
            <kbd className="nav-shortcut">⌘,</kbd>
          </button>
        </div>

        {/* Rescan button */}
        {scanResult && (
          <div className="sidebar-footer">
            <button
              className="rescan-btn"
              onClick={scan}
              disabled={scanning}
            >
              <RefreshCw size={14} className={scanning ? "spinning" : ""} />
              {scanning ? "Scanning..." : "Rescan"}
              <kbd className="nav-shortcut">⌘R</kbd>
            </button>
          </div>
        )}
      </nav>

      {/* Main Content */}
      <main className="main-content">
        {/* Initial scan prompt */}
        {!scanResult && !scanning && view === "dashboard" && !hasOnboarded && (
          <Onboarding onComplete={completeOnboarding} />
        )}

        {/* Post-onboarding scan prompt */}
        {!scanResult && !scanning && view === "dashboard" && hasOnboarded && (
          <div className="scan-prompt">
            <div className="scan-prompt-inner">
              <Search size={48} strokeWidth={1.5} />
              <h2>Ready to Diagnose Your Storage</h2>
              <p>
                RepairIQ will scan your Mac to find where space is being used
                and what's safe to reclaim. Nothing will be deleted without your
                approval.
              </p>
              <button className="scan-btn" onClick={scan}>
                Start Deep Scan
              </button>
              <div className="scan-features">
                <span>✓ No sudo required</span>
                <span>✓ Nothing deleted</span>
                <span>✓ Data stays local</span>
              </div>
            </div>
          </div>
        )}

        {/* Scanning state */}
        {scanning && (
          <div className="scan-prompt">
            <div className="scan-prompt-inner">
              <div className="spinner" />
              <h2>Scanning Your Storage...</h2>
              <p>
                Analyzing Applications, Documents, Downloads, Library, Developer
                folders, Docker, and Caches.
              </p>
              <div className="scan-progress-dots">
                <span className="dot dot-1" />
                <span className="dot dot-2" />
                <span className="dot dot-3" />
              </div>
            </div>
          </div>
        )}

        {/* Error state */}
        {error && (
          <div className="error-banner">
            <p>Scan Error: {error}</p>
            <button onClick={scan}>Retry</button>
          </div>
        )}

        {/* Dashboard View */}
        {!scanning && scanResult && view === "dashboard" && (
          <Dashboard result={scanResult} onShowWhy={() => setView("explorer")} onMoveToVault={requestMoveToVault} onQuickClean={handleBatchClean} />
        )}

        {/* Explorer View */}
        {!scanning && scanResult && view === "explorer" && (
          <div className="explorer-wrapper">
            {/* Treemap Toggle */}
            <div className="explorer-view-toggle">
              <button
                className={`toggle-btn ${!showTreemap ? "active" : ""}`}
                onClick={() => setShowTreemap(false)}
              >
                List View
              </button>
              <button
                className={`toggle-btn ${showTreemap ? "active" : ""}`}
                onClick={() => setShowTreemap(true)}
              >
                Treemap View
              </button>
            </div>

            {showTreemap ? (
              <Treemap
                items={scanResult.items}
                onItemClick={(item) => {
                  setShowTreemap(false);
                  drillDown(item.path);
                }}
              />
            ) : (
              <Explorer
                result={scanResult}
                onMoveToVault={requestMoveToVault}
                onBatchClean={handleBatchClean}
                onDrillDown={drillDown}
                children={children}
                childrenLoading={childrenLoading}
              />
            )}
          </div>
        )}

        {/* Vault View */}
        {view === "vault" && (
          <Vault
            items={vaultItems}
            loading={vaultLoading}
            onRestore={handleRestore}
            onPurge={handlePurge}
            onPurgeAll={handlePurgeAll}
            onDeletePermanently={handleDeletePermanently}
          />
        )}

        {/* Smart Archive View */}
        {view === "archive" && (
          <SmartArchive
            candidates={candidates}
            volumes={volumes}
            loading={archiveLoading}
            archiving={archiving}
            onScan={findCandidates}
            onLoadVolumes={loadVolumes}
            onArchive={handleArchive}
            onToast={addToast}
          />
        )}

        {/* Duplicates View */}
        {view === "duplicates" && <Duplicates />}

        {/* Browsers View */}
        {view === "browsers" && <Browsers />}

        {/* Mac Health View */}
        {view === "health" && <MacHealth />}

        {/* Settings View */}
        {view === "settings" && (
          <Settings settings={settings} onSave={handleSaveSettings} />
        )}
      </main>

      {/* Confirmation Dialog */}
      {confirmAction && (
        <ConfirmDialog
          title="Move to Recovery Vault"
          message={`Move "${confirmAction.item.name}" (${formatBytes(confirmAction.item.size_bytes)}) to the Recovery Vault?`}
          detail={`The item will be recoverable for ${confirmAction.retentionDays} days. You can restore it anytime from the vault.`}
          confirmLabel="Move to Vault"
          variant="warning"
          onConfirm={confirmMoveToVault}
          onCancel={() => setConfirmAction(null)}
        />
      )}

      {/* Batch Clean Confirmation */}
      {batchItems && (
        <ConfirmDialog
          title="Clean All Safe Items"
          message={`Move ${batchItems.length} safe items (${formatBytes(batchItems.reduce((s, i) => s + i.size_bytes, 0))}) to the Recovery Vault?`}
          detail={`All items scored 8/10 or higher for safety. They will be recoverable for ${settings.defaultRetentionDays} days. You can restore any of them from the vault.`}
          confirmLabel={`Clean ${batchItems.length} Items`}
          variant="default"
          onConfirm={confirmBatchClean}
          onCancel={() => setBatchItems(null)}
        />
      )}

      {/* Toast Notifications */}
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}

export default App;
