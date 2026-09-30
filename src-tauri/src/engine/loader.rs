//! 规则加载器 —— 插件化接入的核心。
//!
//! - 内置规则：编译期 include_str! 打包（`src-tauri/rules/*.json`），始终可用
//! - 用户规则：应用数据目录/rules/*.json，随时增删文件即可扩展清理能力，
//!   无需等待软件更新（用户规则与内置规则同 id 时覆盖内置项，便于自定义）
//! - settings.json：持久化规则的启用/禁用状态（用户勾选的“默认不删”）

use std::fs;
use std::path::{Path, PathBuf};

use super::model::{Rule, Settings};

/// (文件名, 内容)：编译期嵌入，随应用分发
pub const BUILTIN_RULES: &[(&str, &str)] = &[
    (
        "temp.json",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/temp.json")),
    ),
    (
        "pkg_caches.json",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/pkg_caches.json")),
    ),
    (
        "updater_caches.json",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/updater_caches.json")),
    ),
    (
        "app_caches.json",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/app_caches.json")),
    ),
];

pub struct LoadedRules {
    pub rules: Vec<Rule>,
    pub warnings: Vec<String>,
}

/// 解析规则文件内容：顶层为对象（单规则）或数组（多规则）均可
fn parse_rules(text: &str) -> Result<Vec<Rule>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    match value {
        serde_json::Value::Array(_) => {
            serde_json::from_value::<Vec<Rule>>(value).map_err(|e| e.to_string())
        }
        serde_json::Value::Object(_) => {
            let rule = serde_json::from_value::<Rule>(value).map_err(|e| e.to_string())?;
            Ok(vec![rule])
        }
        _ => Err("规则文件顶层必须是 JSON 对象或数组".to_string()),
    }
}

/// 加载内置规则 + 用户规则目录。
/// 单条规则解析失败不中断，记入 warnings。
pub fn load_rules(user_rules_dir: Option<&Path>) -> LoadedRules {
    let mut rules: Vec<Rule> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (fname, content) in BUILTIN_RULES {
        match parse_rules(content) {
            Ok(parsed) => {
                for mut rule in parsed {
                    rule.source = "builtin".to_string();
                    rules.push(rule);
                }
            }
            Err(e) => warnings.push(format!("内置规则文件 {fname} 解析失败: {e}")),
        }
    }

    if let Some(dir) = user_rules_dir {
        if let Ok(rd) = fs::read_dir(dir) {
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .map(|x| x.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                })
                .collect();
            files.sort();
            for path in files {
                let result =
                    fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| parse_rules(&s));
                match result {
                    Ok(parsed) => {
                        for mut rule in parsed {
                            rule.source = "user".to_string();
                            let pos = rules.iter().position(|r| r.id == rule.id);
                            match pos {
                                Some(idx) => rules[idx] = rule, // 用户规则覆盖内置项
                                None => rules.push(rule),
                            }
                        }
                    }
                    Err(e) => warnings.push(format!("用户规则 {} 解析失败: {}", path.display(), e)),
                }
            }
        }
    }

    LoadedRules { rules, warnings }
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

pub fn load_settings(data_dir: &Path) -> Settings {
    fs::read_to_string(settings_path(data_dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(data_dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(settings_path(data_dir), text).map_err(|e| e.to_string())
}
