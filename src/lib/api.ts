import { invoke } from "@tauri-apps/api/core";
import type {
  BlockerInfo,
  CleanResult,
  RulesPayload,
  ScanResult,
  Settings,
} from "../types";

export const api = {
  rulesList: () => invoke<RulesPayload>("rules_list"),
  scan: () => invoke<ScanResult>("scan"),
  clean: (paths: string[]) => invoke<CleanResult>("clean", { paths }),
  checkBlockers: (paths: string[]) =>
    invoke<BlockerInfo[]>("check_blockers", { paths }),
  checkAutoruns: (paths: string[]) =>
    invoke<string[]>("check_autoruns", { paths }),
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) =>
    invoke<void>("settings_set", { settings }),
  exportBuiltinRules: () => invoke<string[]>("export_builtin_rules"),
};
