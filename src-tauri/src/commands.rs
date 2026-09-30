//! Tauri 命令层：前端 invoke 的入口。
//!
//! 设计要点：`clean` 在执行前会用当前规则重新发现一遍并校验用户勾选的路径，
//! 过期或伪造的路径一律拒绝，保证「用户勾选什么就只删什么」。

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::Manager;

use crate::engine::checks;
use crate::engine::discover::discover;
use crate::engine::execute::clean_paths;
use crate::engine::loader;
use crate::engine::model::{
    BlockerInfo, CleanResult, RulesPayload, ScanResult, Settings,
};

fn app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| {
        let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        Path::new(&base).join("com.xiaofeiran.cleanmaster")
    })
}

fn user_rules_dir(app: &tauri::AppHandle) -> PathBuf {
    app_data_dir(app).join("rules")
}

/// 列出全部规则（内置 + 用户插件），以及用户规则目录位置与解析告警
#[tauri::command]
pub fn rules_list(app: tauri::AppHandle) -> Result<RulesPayload, String> {
    let dir = user_rules_dir(&app);
    let loaded = loader::load_rules(Some(&dir));
    Ok(RulesPayload {
        rules: loaded.rules,
        warnings: loaded.warnings,
        user_rules_dir: dir.to_string_lossy().to_string(),
    })
}

/// 扫描。`rule_ids` 为 None 时扫描所有未禁用的规则；指定时只扫描给定规则。
#[tauri::command]
pub fn scan(app: tauri::AppHandle, rule_ids: Option<Vec<String>>) -> Result<ScanResult, String> {
    let loaded = loader::load_rules(Some(&user_rules_dir(&app)));
    let settings = loader::load_settings(&app_data_dir(&app));
    let selected: Vec<_> = match &rule_ids {
        Some(ids) => loaded
            .rules
            .into_iter()
            .filter(|r| ids.contains(&r.id))
            .collect(),
        None => loaded
            .rules
            .into_iter()
            .filter(|r| !settings.disabled_rules.contains(&r.id))
            .collect(),
    };
    let discovery = discover(&selected, true);
    let mut items = discovery.items;
    items.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    let total_size = items.iter().map(|i| i.size).sum();
    Ok(ScanResult {
        items,
        total_size: total_size,
        warnings: discovery.warnings,
    })
}

/// 清理用户勾选的路径。执行前重新发现并校验，占用目录自动跳过。
#[tauri::command]
pub fn clean(app: tauri::AppHandle, paths: Vec<String>) -> Result<CleanResult, String> {
    let loaded = loader::load_rules(Some(&user_rules_dir(&app)));
    Ok(clean_paths(&loaded.rules, &paths))
}

/// 检测占用：找出可执行文件位于目标目录内的进程
#[tauri::command]
pub fn check_blockers(paths: Vec<String>) -> Result<Vec<BlockerInfo>, String> {
    let targets: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let map = checks::find_blockers(&targets);
    Ok(map
        .into_iter()
        .map(|(path, processes)| BlockerInfo { path, processes })
        .collect())
}

/// 检测自启动引用：注册表 Run 键 + 计划任务
#[tauri::command]
pub fn check_autoruns(paths: Vec<String>) -> Result<Vec<String>, String> {
    let targets: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    Ok(checks::find_autorun_refs(&targets))
}

#[tauri::command]
pub fn settings_get(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(loader::load_settings(&app_data_dir(&app)))
}

#[tauri::command]
pub fn settings_set(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    loader::save_settings(&app_data_dir(&app), &settings)
}

/// 在资源管理器中打开条目所在文件夹并选中它（explorer /select）。
/// 仅允许打开确实存在的目录；raw_arg 精确控制引号位置（路径可含空格）。
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("路径不存在或不是目录: {path}"));
    }
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", p.display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("打开资源管理器失败: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open")
            .arg(&p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
        Ok(())
    }
}

/// 把内置规则导出为用户目录下的可编辑文件（已存在则跳过，不覆盖用户修改）
#[tauri::command]
pub fn export_builtin_rules(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let dir = user_rules_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut report = Vec::new();
    for (fname, content) in loader::BUILTIN_RULES {
        let dest = dir.join(fname);
        if dest.exists() {
            report.push(format!("{fname}: 已存在，跳过"));
        } else {
            std::fs::write(&dest, content).map_err(|e| e.to_string())?;
            report.push(format!("{fname}: 已导出"));
        }
    }
    report.push(format!("用户规则目录: {}", dir.display()));
    Ok(report)
}
