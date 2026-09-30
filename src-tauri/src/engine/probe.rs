//! 包管理器 CLI 探测（对齐 clean_pkg_caches.py 的 cli_probe）：
//! 运行如 `npm config get cache`，取输出最后一行作为候选缓存路径。
//! Windows 下通过 cmd /C 调用（npm/pnpm/pip/uv 均为 .cmd/.bat 包装器），
//! 并使用 CREATE_NO_WINDOW 避免弹出控制台窗口。

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 运行 CLI 命令，返回 stdout 最后一行（trim 后）；超时或失败返回 None
pub fn probe_cli_path(command: &str, timeout_secs: u64) -> Option<String> {
    let mut parts = command.split_whitespace();
    let exe = parts.next()?;
    let args: Vec<&str> = parts.collect();

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(exe).args(&args);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new(exe);
        c.args(&args);
        c
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() >= Duration::from_secs(timeout_secs) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .trim_end()
        .lines()
        .last()
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}
