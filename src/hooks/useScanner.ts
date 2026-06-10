import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ScanResult, ScannedItem, VaultItem, ArchiveRecommendation } from "../types";

export function useScanner() {
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      const result = await invoke<ScanResult>("scan_storage");
      setScanResult(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  // Remove an item from scan results without rescanning
  const removeItemFromResults = useCallback((path: string) => {
    setScanResult((prev) => {
      if (!prev) return prev;
      const removedItem = prev.items.find((i) => i.path === path);
      if (!removedItem) return prev;

      const newItems = prev.items.filter((i) => i.path !== path);
      const newCategories = prev.categories.map((cat) => ({
        ...cat,
        items: cat.items.filter((i) => i.path !== path),
        size_bytes: cat.items
          .filter((i) => i.path !== path)
          .reduce((sum, i) => sum + i.size_bytes, 0),
      })).filter((cat) => cat.items.length > 0);

      return {
        ...prev,
        items: newItems,
        categories: newCategories,
        free_bytes: prev.free_bytes + removedItem.size_bytes,
        used_bytes: prev.used_bytes - removedItem.size_bytes,
        safe_recovery_bytes: removedItem.safety === "Safe"
          ? prev.safe_recovery_bytes - removedItem.size_bytes
          : prev.safe_recovery_bytes,
        review_recovery_bytes: removedItem.safety === "Review"
          ? prev.review_recovery_bytes - removedItem.size_bytes
          : prev.review_recovery_bytes,
        archive_recovery_bytes: removedItem.safety === "Archive"
          ? prev.archive_recovery_bytes - removedItem.size_bytes
          : prev.archive_recovery_bytes,
      };
    });
  }, []);

  return { scanResult, scanning, error, scan, removeItemFromResults };
}

export function useVault() {
  const [vaultItems, setVaultItems] = useState<VaultItem[]>([]);
  const [loading, setLoading] = useState(false);

  const initVault = useCallback(async () => {
    try {
      await invoke("init_vault");
    } catch (e) {
      console.error("Failed to init vault:", e);
    }
  }, []);

  const moveToVault = useCallback(
    async (path: string, retentionDays: number, category: string) => {
      const item = await invoke<VaultItem>("vault_move", {
        path,
        retentionDays,
        category,
      });
      setVaultItems((prev) => [item, ...prev]);
      return item;
    },
    []
  );

  const restoreFromVault = useCallback(async (id: number) => {
    await invoke("vault_restore", { id });
    setVaultItems((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const listVault = useCallback(async () => {
    setLoading(true);
    try {
      const items = await invoke<VaultItem[]>("vault_list");
      setVaultItems(items);
    } catch (e) {
      console.error("Failed to list vault:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const purgeExpired = useCallback(async () => {
    const freed = await invoke<number>("vault_purge");
    await listVault();
    return freed;
  }, [listVault]);

  return {
    vaultItems,
    loading,
    initVault,
    moveToVault,
    restoreFromVault,
    listVault,
    purgeExpired,
  };
}

export function useExplorer() {
  const [children, setChildren] = useState<ScannedItem[]>([]);
  const [loading, setLoading] = useState(false);

  const drillDown = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const items = await invoke<ScannedItem[]>("get_item_children", { path });
      setChildren(items);
    } catch (e) {
      console.error("Failed to drill down:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  return { children, loading, drillDown };
}

export function useArchive() {
  const [candidates, setCandidates] = useState<ArchiveRecommendation[]>([]);
  const [volumes, setVolumes] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [archiving, setArchiving] = useState(false);

  const findCandidates = useCallback(async () => {
    setLoading(true);
    try {
      const items = await invoke<ArchiveRecommendation[]>("find_archive_candidates");
      setCandidates(items);
    } catch (e) {
      console.error("Failed to find archive candidates:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadVolumes = useCallback(async () => {
    try {
      const vols = await invoke<string[]>("list_volumes");
      setVolumes(vols);
    } catch (e) {
      console.error("Failed to list volumes:", e);
    }
  }, []);

  const archiveProject = useCallback(
    async (sourcePath: string, destinationDir: string) => {
      setArchiving(true);
      try {
        const destPath = await invoke<string>("archive_project", {
          sourcePath,
          destinationDir,
        });
        // Remove from candidates list
        setCandidates((prev) => prev.filter((c) => c.path !== sourcePath));
        return destPath;
      } finally {
        setArchiving(false);
      }
    },
    []
  );

  return {
    candidates,
    volumes,
    loading,
    archiving,
    findCandidates,
    loadVolumes,
    archiveProject,
  };
}
