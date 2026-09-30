//! 扫描引擎：根据规则发现可清理条目。
//!
//! - fixed：环境变量展开后直接命中
//! - drive_roots：枚举盘符根目录下的指定名称目录
//! - pattern：根目录列表 + 名称正则发现，特征文件区分「确认/疑似」
//! - probe：调用包管理器 CLI 获取真实缓存路径，basename/路径关键字校验后采纳
//!
//! 全局按规范化路径去重（先发现的规则优先）；命中保护名单的路径跳过并告警。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::RegexBuilder;

use super::model::{DiscoveredItem, Mode, Rule, Target};
use super::probe;
use super::util;

pub struct Discovery {
    pub items: Vec<DiscoveredItem>,
    pub warnings: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn try_push(
    items: &mut Vec<DiscoveredItem>,
    seen: &mut HashSet<String>,
    warnings: &mut Vec<String>,
    path: PathBuf,
    rule: &Rule,
    mode: Mode,
    status: &str,
    signatures: Vec<String>,
) {
    let key = util::norm(&path);
    if seen.contains(&key) {
        return;
    }
    if util::is_dangerous(&path) {
        warnings.push(format!("已跳过受保护路径: {}", path.display()));
        return;
    }
    seen.insert(key);
    items.push(DiscoveredItem {
        rule_id: rule.id.clone(),
        rule_name: rule.name.clone(),
        category: rule.category.clone(),
        risk: rule.risk.clone(),
        source: rule.source.clone(),
        path: path.to_string_lossy().to_string(),
        size: 0,
        mode,
        status: status.to_string(),
        signatures,
    });
}

/// 发现所有规则的可清理条目。
/// `with_sizes=true` 时逐项计算目录大小（扫描阶段）；
/// `false` 时仅验证存在性（清理前校验，快速）。
pub fn discover(rules: &[Rule], with_sizes: bool) -> Discovery {
    let mut items: Vec<DiscoveredItem> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for rule in rules {
        for target in &rule.targets {
            match target {
                Target::Fixed { path, mode } => {
                    let Some(p) = util::expand_env(path) else {
                        continue;
                    };
                    if !p.is_dir() {
                        continue;
                    }
                    try_push(&mut items, &mut seen, &mut warnings, p, rule, *mode, "normal", vec![]);
                }
                Target::DriveRoots { names, mode } => {
                    for letter in b'A'..=b'Z' {
                        let root = PathBuf::from(format!("{}:\\", letter as char));
                        if !root.is_dir() {
                            continue;
                        }
                        for name in names {
                            let p = root.join(name);
                            if p.is_dir() {
                                try_push(
                                    &mut items, &mut seen, &mut warnings, p, rule, *mode,
                                    "normal", vec![],
                                );
                            }
                        }
                    }
                }
                Target::Pattern {
                    roots,
                    regex,
                    regex_case_insensitive,
                    signature_files,
                    signature_dirs,
                    signature_file_ends_with,
                    mode,
                } => {
                    let re = match RegexBuilder::new(regex)
                        .case_insensitive(*regex_case_insensitive)
                        .build()
                    {
                        Ok(re) => re,
                        Err(e) => {
                            warnings.push(format!("规则 {} 的正则无效: {}", rule.id, e));
                            continue;
                        }
                    };
                    for root_tpl in roots {
                        let Some(root) = util::expand_env(root_tpl) else {
                            continue;
                        };
                        let Ok(rd) = fs::read_dir(&root) else {
                            continue;
                        };
                        for entry in rd.flatten() {
                            let Ok(ft) = entry.file_type() else {
                                continue;
                            };
                            if !ft.is_dir() {
                                continue;
                            }
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !re.is_match(&name) {
                                continue;
                            }
                            let full = entry.path();
                            let sig = dir_signature(
                                &full,
                                signature_files,
                                signature_dirs,
                                signature_file_ends_with,
                            );
                            let status = if sig.is_empty() { "suspected" } else { "confirmed" };
                            try_push(
                                &mut items, &mut seen, &mut warnings, full, rule, *mode, status,
                                sig,
                            );
                        }
                    }
                }
                Target::Probe {
                    command,
                    safe_basenames,
                    path_contains_any,
                    mode,
                } => {
                    let Some(line) = probe::probe_cli_path(command, 20) else {
                        continue;
                    };
                    let p = PathBuf::from(&line);
                    if !p.is_dir() {
                        continue;
                    }
                    let base_ok = p.file_name().map(|f| {
                        let fb = f.to_string_lossy().to_lowercase();
                        safe_basenames.iter().any(|s| s.to_lowercase() == fb)
                    });
                    let contains_ok = {
                        let pl = util::norm(&p);
                        path_contains_any.iter().any(|s| pl.contains(&s.to_lowercase()))
                    };
                    if !base_ok.unwrap_or(false) && !contains_ok {
                        continue;
                    }
                    try_push(&mut items, &mut seen, &mut warnings, p, rule, *mode, "normal", vec![]);
                }
            }
        }
    }

    if with_sizes {
        for item in &mut items {
            item.size = util::dir_size(Path::new(&item.path));
        }
    }

    Discovery { items, warnings }
}

/// 检查目录内的特征文件/子目录（对齐 clean_updater_cache.py 的 dir_signature）
fn dir_signature(
    path: &Path,
    sig_files: &[String],
    sig_dirs: &[String],
    ends_with: &[String],
) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    let Ok(rd) = fs::read_dir(path) else {
        return hits;
    };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let lname = name.to_lowercase();
        if ft.is_dir() {
            if sig_dirs.iter().any(|d| d.to_lowercase() == lname) {
                hits.push(format!("{name}/"));
            }
        } else if ft.is_file() {
            let file_hit = sig_files.iter().any(|f| f.to_lowercase() == lname)
                || ends_with.iter().any(|s| lname.ends_with(&s.to_lowercase()));
            if file_hit {
                hits.push(name);
            }
        }
    }
    hits
}
