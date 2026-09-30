//! 附加检测：进程占用与自启动引用（对齐 clean_updater_cache.py）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::util::norm;

/// 找出可执行文件位于目标目录内的进程（等价于脚本中 psutil 的作用）。
/// 返回 规范化目录路径 -> 占用进程描述列表。
pub fn find_blockers(targets: &[PathBuf]) -> HashMap<String, Vec<String>> {
    let mut blockers: HashMap<String, Vec<String>> = HashMap::new();
    if targets.is_empty() {
        return blockers;
    }
    let sys = sysinfo::System::new_all();
    for (pid, process) in sys.processes() {
        let Some(exe) = process.exe() else { continue };
        for t in targets {
            // 按路径组件判断 exe 是否位于目标目录内（避免字符串前缀误判）
            if exe.starts_with(t) {
                blockers.entry(norm(t)).or_default().push(format!(
                    "{} (PID {})",
                    process.name().to_string_lossy(),
                    pid.as_u32()
                ));
            }
        }
    }
    blockers
}

/// 在注册表 Run 键与计划任务 XML 中查找引用目标目录的启动项（仅 Windows 有效）。
pub fn find_autorun_refs(targets: &[PathBuf]) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    if targets.is_empty() {
        return refs;
    }
    let tnorms: Vec<String> = targets.iter().map(|p| norm(p)).collect();

    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        // 1) 注册表 Run 键
        let run_key = r"Software\Microsoft\Windows\CurrentVersion\Run";
        for (label, hive) in [("HKCU", HKEY_CURRENT_USER), ("HKLM", HKEY_LOCAL_MACHINE)] {
            // HKLM 无权限时跳过，不中断
            let Ok(key) = RegKey::predef(hive).open_subkey(run_key) else {
                continue;
            };
            for (name, value) in key.enum_values().flatten() {
                let value_str = value.to_string();
                let vn = value_str.to_lowercase();
                if tnorms.iter().any(|tn| vn.contains(tn)) {
                    refs.push(format!("[注册表-{label}\\...\\Run] {name} = {value_str}"));
                }
            }
        }

        // 2) 计划任务：任务 XML 为 UTF-16LE 编码，扫描内容是否包含目标路径
        let windir =
            std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
        let mut stack = vec![tasks_dir];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for entry in rd.flatten() {
                let Ok(ft) = entry.file_type() else { continue };
                let p = entry.path();
                if ft.is_dir() {
                    stack.push(p);
                    continue;
                }
                let Ok(raw) = fs::read(&p) else { continue };
                let content = decode_utf16le_lossy(&raw).to_lowercase();
                for tn in &tnorms {
                    if content.contains(tn) {
                        refs.push(format!(
                            "[计划任务] {}  ->  {}",
                            entry.file_name().to_string_lossy(),
                            tn
                        ));
                        break;
                    }
                }
            }
        }
    }

    let _ = &tnorms; // 非 Windows 平台下避免未使用告警
    refs
}

fn decode_utf16le_lossy(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}
