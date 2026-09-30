//! 引擎公共工具：环境变量展开、目录大小统计、只读感知的强制删除、安全保护名单。
//!
//! 删除语义对齐原始三个 Python 脚本：
//! - 遇到只读文件先清除属性再删（Squirrel 缓存文件常带只读）
//! - 被占用/无权限的文件与目录自动跳过，不中断整体流程
//! - `clean_contents` 仅清空目录内容，保留目录本身（对应 %TEMP% 清理）

use std::fs;
use std::path::{Path, PathBuf};

/// 用户主目录（%USERPROFILE%）
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// 展开 `~` 与 `%VAR%`。
/// - 变量未定义时返回 None（该目标视为不可用，与 Python 中 os.environ.get 缺失跳过一致）
/// - %SystemRoot% 缺省回退 C:\Windows（对齐脚本行为）
pub fn expand_env(template: &str) -> Option<PathBuf> {
    let raw = template.trim();
    if raw.is_empty() {
        return None;
    }
    let mut s = if let Some(rest) = raw.strip_prefix('~') {
        let h = home_dir()?;
        format!("{}{}", h.to_string_lossy(), rest)
    } else {
        raw.to_string()
    };

    if !s.contains('%') {
        return Some(PathBuf::from(s));
    }

    let mut out = String::new();
    let mut rest: &str = s.as_str();
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('%') {
            Some(j) => {
                let var = &after[..j];
                let val = if var.eq_ignore_ascii_case("SystemRoot") {
                    std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string())
                } else {
                    match std::env::var(var) {
                        Ok(v) => v,
                        Err(_) => return None,
                    }
                };
                if val.trim().is_empty() {
                    return None;
                }
                out.push_str(&val);
                rest = &after[j + 1..];
            }
            None => {
                // 无闭合 '%'：按字面量处理
                out.push('%');
                out.push_str(after);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    s = out;

    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// 规范化路径用于比较（对齐 Python os.path.normcase：小写 + 统一分隔符）
pub fn norm(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// 递归统计目录大小，跳过符号链接与无权限项（对齐 os.walk(onerror=None)）
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(entry.path());
            } else if let Ok(md) = entry.metadata() {
                total += md.len();
            }
        }
    }
    total
}

/// 清除 Windows 只读属性（其余平台为空操作）
pub fn clear_readonly(p: &Path) {
    #[cfg(windows)]
    if let Ok(md) = fs::metadata(p) {
        let mut perms = md.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(p, perms);
        }
    }
}

/// 强制删除单个文件（先去只读），失败返回 false
pub fn force_remove_file(p: &Path) -> bool {
    clear_readonly(p);
    fs::remove_file(p).is_ok()
}

/// 自底向上强制删除整个目录：先删文件（去只读），再按深度从深到浅删目录。
/// 返回是否全部成功；被占用目录会删除失败并返回 false（不中断其他目标）。
pub fn force_rmtree(path: &Path) -> bool {
    let mut ok = true;
    let mut dirs: Vec<(PathBuf, usize)> = vec![(path.to_path_buf(), 0)];
    let mut stack: Vec<(PathBuf, usize)> = vec![(path.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            ok = false;
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                ok = false;
                continue;
            };
            let p = entry.path();
            if ft.is_symlink() {
                if fs::remove_file(&p).is_err() {
                    ok = false;
                }
            } else if ft.is_dir() {
                dirs.push((p.clone(), depth + 1));
                stack.push((p, depth + 1));
            } else if !force_remove_file(&p) {
                ok = false;
            }
        }
    }
    dirs.sort_by(|a, b| b.1.cmp(&a.1));
    for (d, _) in dirs {
        if fs::remove_dir(&d).is_err() {
            ok = false;
        }
    }
    ok
}

pub struct ContentsStats {
    pub freed: u64,
    pub deleted_files: u64,
    pub skipped_files: u64,
}

/// 清空目录内容但保留目录本身（对齐 clean_appdata_temp.py）：
/// 文件逐个删除并统计 freed/deleted/skipped；空目录顺带删除，非空目录保留。
pub fn clean_contents(path: &Path) -> ContentsStats {
    let mut stats = ContentsStats {
        freed: 0,
        deleted_files: 0,
        skipped_files: 0,
    };
    let mut dirs: Vec<(PathBuf, usize)> = vec![];
    let mut stack: Vec<(PathBuf, usize)> = vec![(path.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                stats.skipped_files += 1;
                continue;
            };
            let p = entry.path();
            if ft.is_symlink() {
                if fs::remove_file(&p).is_ok() {
                    stats.deleted_files += 1;
                } else {
                    stats.skipped_files += 1;
                }
            } else if ft.is_dir() {
                dirs.push((p.clone(), depth + 1));
                stack.push((p, depth + 1));
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if force_remove_file(&p) {
                    stats.freed += size;
                    stats.deleted_files += 1;
                } else {
                    stats.skipped_files += 1;
                }
            }
        }
    }
    dirs.sort_by(|a, b| b.1.cmp(&a.1));
    for (d, _) in dirs {
        // 只有空目录能删掉，失败即保留（对齐 os.rmdir 语义）
        let _ = fs::remove_dir(&d);
    }
    stats
}

/// 引擎级安全保护：拒绝删除以下路径
/// - 盘符根目录（C:\ 等）
/// - 用户主目录、%LOCALAPPDATA%、%APPDATA%、%ProgramData%、%ProgramFiles%、%SystemRoot% 等系统根
/// - %LOCALAPPDATA%\pnpm（对齐脚本 is_dangerous）
/// - 上述目录的祖先目录（防止误删 C:\Users 之类上层目录）
pub fn is_dangerous(path: &Path) -> bool {
    let n = norm(path);
    if n.is_empty() {
        return true;
    }
    // 盘符根目录
    let stripped = n.strip_suffix('\\').unwrap_or(&n);
    if stripped.len() == 2 && stripped.ends_with(':') {
        return true;
    }

    let mut protected: Vec<PathBuf> = vec![];
    if let Some(h) = home_dir() {
        protected.push(h);
    }
    for var in [
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "SystemRoot",
    ] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                protected.push(PathBuf::from(v));
            }
        }
    }
    if let Ok(la) = std::env::var("LOCALAPPDATA") {
        protected.push(Path::new(&la).join("pnpm"));
    }
    for p in &protected {
        if norm(p) == n {
            return true;
        }
    }
    // 用户主目录的祖先目录（含其自身，已在上面精确匹配）一律拒绝
    if let Some(h) = home_dir() {
        if h.starts_with(path) {
            return true;
        }
    }
    false
}
