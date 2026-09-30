//! 清理执行引擎：按用户勾选的路径清理，执行前重新校验。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::checks;
use super::discover::discover;
use super::model::{CleanResult, DiscoveredItem, ItemResult, Mode, Rule};
use super::util;

/// 清理指定路径。安全校验：
/// 1. 路径必须出现在当前规则的重新发现结果中（防止过期/伪造的 UI 状态误删）
/// 2. 命中引擎保护名单直接拒绝
/// 3. whole 模式删除前做进程占用检测，占用中的目录自动跳过
/// 4. contents 模式逐文件删除并统计跳过数（对齐 clean_appdata_temp.py）
pub fn clean_paths(rules: &[Rule], requested: &[String]) -> CleanResult {
    let mut result = CleanResult::default();
    if requested.is_empty() {
        return result;
    }

    let discovery = discover(rules, false);
    let index: HashMap<String, &DiscoveredItem> = discovery
        .items
        .iter()
        .map(|it| (util::norm(Path::new(&it.path)), it))
        .collect();

    // whole 模式目标的进程占用检测（对齐 clean_updater_cache.py 的行为）
    let req_paths: Vec<PathBuf> = requested.iter().map(PathBuf::from).collect();
    let whole_norms: HashSet<String> = req_paths
        .iter()
        .filter_map(|p| index.get(&util::norm(p)))
        .filter(|it| it.mode == Mode::Whole)
        .map(|it| util::norm(Path::new(&it.path)))
        .collect();
    let blockers = checks::find_blockers(&req_paths);
    let _ = whole_norms; // 保留语义标注：占用跳过仅应用于 whole 模式目标

    for req in requested {
        let path = PathBuf::from(req);
        let key = util::norm(&path);

        let Some(item) = index.get(&key).copied() else {
            result.failed += 1;
            result.results.push(ItemResult {
                path: req.clone(),
                ok: false,
                action: "not_found".into(),
                freed: 0,
                deleted_files: 0,
                skipped_files: 0,
                message: "路径不在当前规则的发现结果中，已拒绝删除".into(),
            });
            continue;
        };

        if util::is_dangerous(&path) {
            result.failed += 1;
            result.results.push(ItemResult {
                path: req.clone(),
                ok: false,
                action: "refused".into(),
                freed: 0,
                deleted_files: 0,
                skipped_files: 0,
                message: "命中引擎保护名单，已拒绝删除".into(),
            });
            continue;
        }

        match item.mode {
            Mode::Contents => {
                if !path.is_dir() {
                    result.results.push(ItemResult {
                        path: req.clone(),
                        ok: true,
                        action: "not_found".into(),
                        freed: 0,
                        deleted_files: 0,
                        skipped_files: 0,
                        message: "目录不存在（可能已被清理）".into(),
                    });
                    continue;
                }
                let stats = util::clean_contents(&path);
                result.freed += stats.freed;
                result.deleted += 1;
                result.results.push(ItemResult {
                    path: req.clone(),
                    ok: true,
                    action: "cleaned_contents".into(),
                    freed: stats.freed,
                    deleted_files: stats.deleted_files,
                    skipped_files: stats.skipped_files,
                    message: format!(
                        "已清空内容：删除 {} 个文件，跳过 {} 个（占用/无权限）",
                        stats.deleted_files, stats.skipped_files
                    ),
                });
            }
            Mode::Whole => {
                if !path.is_dir() {
                    result.results.push(ItemResult {
                        path: req.clone(),
                        ok: true,
                        action: "not_found".into(),
                        freed: 0,
                        deleted_files: 0,
                        skipped_files: 0,
                        message: "目录不存在（可能已被清理）".into(),
                    });
                    continue;
                }
                if blockers.get(&key).map(|v| !v.is_empty()).unwrap_or(false) {
                    result.failed += 1;
                    result.results.push(ItemResult {
                        path: req.clone(),
                        ok: false,
                        action: "skipped_occupied".into(),
                        freed: 0,
                        deleted_files: 0,
                        skipped_files: 0,
                        message: format!(
                            "目录被进程占用，已跳过：{}。关闭对应软件后重试。",
                            blockers
                                .get(&key)
                                .map(|v| v.join(", "))
                                .unwrap_or_default()
                        ),
                    });
                    continue;
                }
                let size = util::dir_size(&path);
                if util::force_rmtree(&path) {
                    result.freed += size;
                    result.deleted += 1;
                    result.results.push(ItemResult {
                        path: req.clone(),
                        ok: true,
                        action: "deleted".into(),
                        freed: size,
                        deleted_files: 0,
                        skipped_files: 0,
                        message: "已删除".into(),
                    });
                } else {
                    result.failed += 1;
                    result.results.push(ItemResult {
                        path: req.clone(),
                        ok: false,
                        action: "failed".into(),
                        freed: 0,
                        deleted_files: 0,
                        skipped_files: 0,
                        message: "部分文件被占用或无权限，删除未完成".into(),
                    });
                }
            }
        }
    }

    result
}
