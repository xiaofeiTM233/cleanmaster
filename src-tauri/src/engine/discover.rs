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
                    keep_latest,
                    unconfirmed_status,
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
                        // 先收集当前 root 下所有命中项，keep_latest 需要整体排序
                        let mut matched: Vec<(PathBuf, Vec<String>)> = Vec::new();
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
                            matched.push((full, sig));
                        }
                        let selected =
                            apply_keep_latest(&mut matched, *keep_latest, &mut warnings);
                        for (full, sig) in selected {
                            let status = if sig.is_empty() {
                                unconfirmed_status.as_str()
                            } else {
                                "confirmed"
                            };
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

/// keep_latest 语义：按目录名中的版本号排序，仅保留最新的 `n` 个（n=0 时全部保留）。
/// 无法解析出版本号的目录被保守丢弃（不进入清理候选），避免误删命名不规范的在用目录。
fn apply_keep_latest(
    matched: &mut Vec<(PathBuf, Vec<String>)>,
    n: u32,
    warnings: &mut Vec<String>,
) -> Vec<(PathBuf, Vec<String>)> {
    if n == 0 {
        return matched.clone();
    }
    let mut keyed: Vec<(Vec<u64>, PathBuf, Vec<String>)> = matched
        .iter()
        .filter_map(|(p, sig)| version_key(&file_name_of(p)).map(|k| (k, p.clone(), sig.clone())))
        .collect();
    let skipped = matched.len() - keyed.len();
    if skipped > 0 {
        warnings.push(format!(
            "keep_latest: {} 个目录名无法解析出版本号，已保守跳过",
            skipped
        ));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0)); // Vec<u64> 字典序：[2,9] < [2,9,0]，语义正确
    let skip = keyed.len().saturating_sub(n as usize);
    keyed
        .into_iter()
        .skip(skip)
        .map(|(_, p, sig)| (p, sig))
        .collect()
}

fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 从目录名提取版本号向量：定位名称中第一个「数字(.数字)+」段并逐段解析。
/// - "app-2.9.4"        -> [2, 9, 4]
/// - "2.5.2-1a2b3c"     -> [2, 5, 2]（hash 中的数字不参与）
/// - "v3.10.1"          -> [3, 10, 1]
/// - "abc" / "2-hash"   -> None（单段数字或无数字，视为不可解析）
fn version_key(name: &str) -> Option<Vec<u64>> {
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() && !b[i].is_ascii_digit() {
        i += 1;
    }
    let mut segs: Vec<u64> = Vec::new();
    while i < b.len() && b[i].is_ascii_digit() {
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let mut v: u64 = 0;
        for &d in &b[start..i] {
            v = v.saturating_mul(10).saturating_add((d - b'0') as u64);
        }
        segs.push(v);
        if i < b.len() && b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
            i += 1;
        } else {
            break;
        }
    }
    if segs.len() >= 2 {
        Some(segs)
    } else {
        None
    }
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
