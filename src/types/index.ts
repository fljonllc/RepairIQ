export type SafetyLevel = "Safe" | "Review" | "Archive" | "Protected";

export interface ScannedItem {
  path: string;
  name: string;
  size_bytes: number;
  category: string;
  subcategory: string;
  safety: SafetyLevel;
  safety_score: number;
  last_accessed_days: number | null;
  description: string;
  impact: string;
  recovery_method: string;
  owner: string;
  verdict: string;
  verdict_reason: string;
  file_count: number;
  largest_files: string[];
  depends_on: string[];
  clean_command: string;
  recommendation: string;
  action_label: string;
  risk_level: string;
  time_to_rebuild: string;
  side_effects: string;
  why_here: string;
  reasoning: string[];
  confidence: number;
  evidence: string[];
  why_recommended: string;
  what_if_wrong: string;
}

export interface CategoryBreakdown {
  name: string;
  size_bytes: number;
  items: ScannedItem[];
}

export interface ScanResult {
  total_bytes: number;
  used_bytes: number;
  free_bytes: number;
  safe_recovery_bytes: number;
  review_recovery_bytes: number;
  archive_recovery_bytes: number;
  categories: CategoryBreakdown[];
  items: ScannedItem[];
  scan_duration_ms: number;
  health_score: number;
  health_grade: string;
  health_factors: string[];
}

export interface VaultItem {
  id: number;
  original_path: string;
  vault_path: string;
  name: string;
  size_bytes: number;
  moved_at: string;
  expires_at: string;
  category: string;
}

export interface ArchiveRecommendation {
  path: string;
  name: string;
  size_bytes: number;
  last_opened_days: number;
  project_type: string;
  status: string;
}

export interface Toast {
  id: string;
  type: "success" | "error" | "info";
  message: string;
}

export type View = "dashboard" | "explorer" | "vault" | "archive" | "duplicates" | "browsers" | "settings";

export interface DuplicateGroup {
  hash: string;
  file_name: string;
  size_bytes: number;
  count: number;
  total_wasted: number;
  paths: string[];
}

export interface BrowserCache {
  browser: string;
  size_bytes: number;
  path: string;
  clean_command: string;
}

export interface StorageForecast {
  history: StorageSnapshot[];
  days_until_full: number | null;
  daily_growth_bytes: number;
  weekly_growth_bytes: number;
  total_cleaned_bytes: number;
  total_cleaned_count: number;
}

export interface StorageSnapshot {
  date: string;
  used_bytes: number;
  free_bytes: number;
  total_bytes: number;
}
