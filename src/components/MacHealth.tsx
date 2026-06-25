import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Cpu, Battery, Wifi, Shield, HardDrive, Clock } from "lucide-react";
import { formatBytes } from "../utils";

// Inline types for the new modules
interface StartupItem { name: string; path: string; item_type: string; enabled: boolean; necessary: boolean; reason: string; }
interface LargeFile { path: string; name: string; size_bytes: number; last_accessed_days: number; file_type: string; }
interface OldApp { name: string; path: string; size_bytes: number; last_opened_days: number; recommendation: string; }
interface PrivacyItem { category: string; app: string; description: string; size_bytes: number; path: string; risk_level: string; }
interface NetworkConn { process_name: string; pid: string; remote_address: string; status: string; }
interface BatteryInfo { cycle_count: number; max_capacity_percent: number; condition: string; is_charging: boolean; current_charge_percent: number; health_grade: string; recommendation: string; }
interface MemoryInfo { total_bytes: number; used_bytes: number; free_bytes: number; pressure: string; top_consumers: { name: string; memory_bytes: number; pid: number }[]; }

type HealthTab = "overview" | "startup" | "largefiles" | "oldapps" | "privacy" | "network" | "battery" | "memory";

export function MacHealth() {
  const [tab, setTab] = useState<HealthTab>("overview");
  const [battery, setBattery] = useState<BatteryInfo | null>(null);
  const [memory, setMemory] = useState<MemoryInfo | null>(null);
  const [startup, setStartup] = useState<StartupItem[]>([]);
  const [largeFiles, setLargeFiles] = useState<LargeFile[]>([]);
  const [oldApps, setOldApps] = useState<OldApp[]>([]);
  const [privacy, setPrivacy] = useState<PrivacyItem[]>([]);
  const [network, setNetwork] = useState<NetworkConn[]>([]);
  const [loading, setLoading] = useState(false);

  const loadAll = async () => {
    setLoading(true);
    try {
      const [bat, mem, start, large, old, priv, net] = await Promise.all([
        invoke<BatteryInfo>("get_battery_info"),
        invoke<MemoryInfo>("get_memory_info"),
        invoke<StartupItem[]>("get_startup_items"),
        invoke<LargeFile[]>("find_large_files", { minSizeMb: 100 }),
        invoke<OldApp[]>("find_old_apps", { daysThreshold: 90 }),
        invoke<PrivacyItem[]>("scan_privacy"),
        invoke<NetworkConn[]>("get_network_connections"),
      ]);
      setBattery(bat);
      setMemory(mem);
      setStartup(start);
      setLargeFiles(large);
      setOldApps(old);
      setPrivacy(priv);
      setNetwork(net);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  useEffect(() => { loadAll(); }, []);

  const tabs: { id: HealthTab; label: string; icon: React.ComponentType<{ size?: number }> }[] = [
    { id: "overview", label: "Overview", icon: Activity },
    { id: "startup", label: "Startup", icon: Clock },
    { id: "memory", label: "Memory", icon: Cpu },
    { id: "battery", label: "Battery", icon: Battery },
    { id: "network", label: "Network", icon: Wifi },
    { id: "privacy", label: "Privacy", icon: Shield },
    { id: "largefiles", label: "Large Files", icon: HardDrive },
    { id: "oldapps", label: "Old Apps", icon: HardDrive },
  ];

  return (
    <div className="mac-health">
      <div className="health-tabs">
        {tabs.map((t) => (
          <button key={t.id} className={`health-tab ${tab === t.id ? "active" : ""}`} onClick={() => setTab(t.id)}>
            <t.icon size={14} /> {t.label}
          </button>
        ))}
      </div>

      {loading && <div className="loading">Analyzing system health...</div>}

      {!loading && tab === "overview" && battery && memory && (
        <div className="health-overview">
          <div className="health-card">
            <Battery size={18} />
            <div><strong>Battery: {battery.health_grade}</strong><br/>{battery.recommendation}</div>
          </div>
          <div className="health-card">
            <Cpu size={18} />
            <div><strong>Memory: {memory.pressure}</strong><br/>{formatBytes(memory.used_bytes)} of {formatBytes(memory.total_bytes)} used</div>
          </div>
          <div className="health-card">
            <Clock size={18} />
            <div><strong>Startup Items: {startup.length}</strong><br/>{startup.filter(s => !s.necessary).length} third-party items slowing boot</div>
          </div>
          <div className="health-card">
            <Wifi size={18} />
            <div><strong>Network: {network.length} connections</strong><br/>{network.length} apps communicating right now</div>
          </div>
          <div className="health-card">
            <Shield size={18} />
            <div><strong>Privacy: {privacy.length} items</strong><br/>{privacy.filter(p => p.risk_level === "High").length} high-risk tracking items</div>
          </div>
          <div className="health-card">
            <HardDrive size={18} />
            <div><strong>Large Files: {largeFiles.length}</strong><br/>{formatBytes(largeFiles.reduce((s, f) => s + f.size_bytes, 0))} in large files</div>
          </div>
        </div>
      )}

      {!loading && tab === "startup" && (
        <div className="health-list">
          <h3>Startup Items ({startup.length})</h3>
          {startup.map((item, i) => (
            <div key={i} className={`health-row ${item.necessary ? "" : "health-row-warn"}`}>
              <span className="health-row-name">{item.name}</span>
              <span className="health-row-type">{item.item_type}</span>
              <span className="health-row-reason">{item.reason}</span>
            </div>
          ))}
        </div>
      )}

      {!loading && tab === "memory" && memory && (
        <div className="health-list">
          <h3>Memory Pressure: {memory.pressure}</h3>
          <p className="health-sub">{formatBytes(memory.used_bytes)} used of {formatBytes(memory.total_bytes)}</p>
          {memory.top_consumers.map((proc, i) => (
            <div key={i} className="health-row">
              <span className="health-row-name">{proc.name}</span>
              <span className="health-row-size">{formatBytes(proc.memory_bytes)}</span>
            </div>
          ))}
        </div>
      )}

      {!loading && tab === "battery" && battery && (
        <div className="health-list">
          <h3>Battery Health: {battery.health_grade}</h3>
          <div className="battery-stats">
            <span>Charge: {battery.current_charge_percent}%</span>
            <span>Capacity: {battery.max_capacity_percent}%</span>
            <span>Cycles: {battery.cycle_count}</span>
            <span>Condition: {battery.condition}</span>
            <span>{battery.is_charging ? "⚡ Charging" : "🔋 On Battery"}</span>
          </div>
          <p className="health-recommendation">{battery.recommendation}</p>
        </div>
      )}

      {!loading && tab === "network" && (
        <div className="health-list">
          <h3>Active Network Connections ({network.length})</h3>
          {network.map((conn, i) => (
            <div key={i} className="health-row">
              <span className="health-row-name">{conn.process_name}</span>
              <span className="health-row-addr">{conn.remote_address}</span>
              <span className="health-row-status">{conn.status}</span>
            </div>
          ))}
        </div>
      )}

      {!loading && tab === "privacy" && (
        <div className="health-list">
          <h3>Privacy Items ({privacy.length})</h3>
          {privacy.map((item, i) => (
            <div key={i} className={`health-row health-risk-${item.risk_level.toLowerCase()}`}>
              <span className="health-row-name">{item.app} — {item.description}</span>
              <span className="health-row-size">{formatBytes(item.size_bytes)}</span>
              <span className="health-row-risk">{item.risk_level}</span>
            </div>
          ))}
        </div>
      )}

      {!loading && tab === "largefiles" && (
        <div className="health-list">
          <h3>Largest Files ({largeFiles.length})</h3>
          {largeFiles.map((file, i) => (
            <div key={i} className="health-row">
              <span className="health-row-name">{file.name}</span>
              <span className="health-row-type">{file.file_type}</span>
              <span className="health-row-size">{formatBytes(file.size_bytes)}</span>
              <span className="health-row-days">{file.last_accessed_days}d ago</span>
            </div>
          ))}
        </div>
      )}

      {!loading && tab === "oldapps" && (
        <div className="health-list">
          <h3>Old Applications ({oldApps.length})</h3>
          {oldApps.map((app, i) => (
            <div key={i} className="health-row">
              <span className="health-row-name">{app.name}</span>
              <span className="health-row-size">{formatBytes(app.size_bytes)}</span>
              <span className="health-row-days">{app.last_opened_days}d since opened</span>
              <span className="health-row-rec">{app.recommendation}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
