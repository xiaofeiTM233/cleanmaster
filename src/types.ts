// 与 Rust 侧 serde DTO 一一对应（camelCase）

export type Mode = "contents" | "whole";
export type ItemStatus = "normal" | "confirmed" | "suspected";

export interface Rule {
  schema: number;
  id: string;
  name: string;
  category: string;
  description: string;
  risk: string;
  defaultEnabled: boolean;
  source: string; // builtin | user
  targets: unknown[];
}

export interface DiscoveredItem {
  ruleId: string;
  ruleName: string;
  category: string;
  risk: string;
  source: string;
  path: string;
  size: number;
  mode: Mode;
  status: ItemStatus;
  signatures: string[];
}

export interface ScanResult {
  items: DiscoveredItem[];
  totalSize: number;
  warnings: string[];
}

export interface ItemResult {
  path: string;
  ok: boolean;
  action: string;
  freed: number;
  deletedFiles: number;
  skippedFiles: number;
  message: string;
}

export interface CleanResult {
  freed: number;
  deleted: number;
  failed: number;
  results: ItemResult[];
}

export interface BlockerInfo {
  path: string;
  processes: string[];
}

export interface Settings {
  disabledRules: string[];
}

export interface RulesPayload {
  rules: Rule[];
  warnings: string[];
  userRulesDir: string;
}
