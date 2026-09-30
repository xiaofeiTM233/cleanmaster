//! CleanMaster 清理规则引擎
//!
//! 规则以声明式 JSON 文件描述：
//! - 内置默认规则随应用编译打包（`src-tauri/rules/*.json`）
//! - 用户规则目录（应用数据目录/rules/*.json）可随时增改，无需重新编译
//! - 引擎负责发现（discover）、安全校验与清理（execute）
//!
//! 目标类型（Target kind）：
//! - `fixed`       固定路径（支持 %ENV% 与 ~ 展开）
//! - `drive_roots` 扫描所有盘符根目录下的指定名称目录（如 E:\.pnpm-store）
//! - `pattern`     在若干根目录下按名称正则发现目录，可用特征文件确认身份
//! - `probe`       调用包管理器 CLI 获取真实缓存路径

#![allow(dead_code)] // TODO: 命令层接通后移除

pub mod model;
pub mod probe;
pub mod util;
