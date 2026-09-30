//! 规则数据模型：JSON 规则文件与序列化 DTO 的统一类型定义。

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}
fn default_risk() -> String {
    "low".to_string()
}
fn default_schema() -> u32 {
    1
}
fn default_mode() -> Mode {
    Mode::Whole
}
fn default_suspected() -> String {
    "suspected".to_string()
}

/// 删除模式
/// - `whole`:    整个目录强制删除（包管理器缓存 / 更新缓存）
/// - `contents`: 仅清空目录内容、保留目录本身（%TEMP% 临时清理）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Contents,
    Whole,
}

/// 一条清理规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    #[serde(default = "default_schema")]
    pub schema: u32,
    /// 全局唯一 id，如 "pkg-caches.npm"
    pub id: String,
    pub name: String,
    /// 分类（临时文件 / 包管理器缓存 / 开发工具缓存 / 应用更新缓存 ...）
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// low / medium / high，仅作 UI 提示
    #[serde(default = "default_risk")]
    pub risk: String,
    /// UI 中默认是否启用
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    /// 规则来源：builtin / user（由加载器写入，不来自 JSON）
    #[serde(default)]
    pub source: String,
    pub targets: Vec<Target>,
}

/// 清理目标。`kind` 为 JSON 中的判别字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Target {
    /// 固定路径
    #[serde(rename = "fixed")]
    Fixed {
        path: String,
        #[serde(default = "default_mode")]
        mode: Mode,
    },
    /// 扫描所有盘符根目录下的指定名称目录（如 E:\.pnpm-store）
    #[serde(rename = "drive_roots")]
    DriveRoots {
        names: Vec<String>,
        #[serde(default = "default_mode")]
        mode: Mode,
    },
    /// 根目录 + 名称正则发现（如 *-updater），支持特征文件确认
    #[serde(rename = "pattern")]
    Pattern {
        roots: Vec<String>,
        regex: String,
        #[serde(default = "default_true")]
        regex_case_insensitive: bool,
        /// 命中任意一个即视为“确认”目录（文件名，不区分大小写）
        #[serde(default)]
        signature_files: Vec<String>,
        /// 特征子目录（目录名，不区分大小写）
        #[serde(default)]
        signature_dirs: Vec<String>,
        /// 文件名以任意后缀结尾即命中（如 .nupkg）
        #[serde(default)]
        signature_file_ends_with: Vec<String>,
        /// 多版本目录模式：按目录名中的版本号排序，仅保留最新的 N 个（0 = 不限制）。
        /// 无法从目录名解析出版本号（形如 x.y）的目录将被保守跳过，绝不删除。
        #[serde(default)]
        keep_latest: u32,
        /// 无特征命中时条目的状态标记（默认 suspected；对目录名精确匹配的
        /// 缓存子目录可设为 normal 以便默认勾选）
        #[serde(default = "default_suspected")]
        unconfirmed_status: String,
        #[serde(default = "default_mode")]
        mode: Mode,
    },
    /// 调用包管理器 CLI 获取真实缓存路径
    #[serde(rename = "probe")]
    Probe {
        command: String,
        /// 输出路径的 basename 必须在其中（不区分大小写）才采纳
        #[serde(default)]
        safe_basenames: Vec<String>,
        /// 或完整路径包含任一关键字才采纳
        #[serde(default)]
        path_contains_any: Vec<String>,
        #[serde(default = "default_mode")]
        mode: Mode,
    },
}

/// 扫描发现的一个可清理条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredItem {
    pub rule_id: String,
    pub rule_name: String,
    pub category: String,
    pub risk: String,
    pub source: String,
    pub path: String,
    pub size: u64,
    pub mode: Mode,
    /// normal | confirmed | suspected
    pub status: String,
    /// pattern 目标命中的特征文件/目录
    pub signatures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub items: Vec<DiscoveredItem>,
    pub total_size: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub path: String,
    pub ok: bool,
    /// deleted | cleaned_contents | skipped_occupied | refused | not_found | failed
    pub action: String,
    pub freed: u64,
    pub deleted_files: u64,
    pub skipped_files: u64,
    pub message: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanResult {
    pub freed: u64,
    pub deleted: usize,
    pub failed: usize,
    pub results: Vec<ItemResult>,
}

/// 用户设置（持久化到应用数据目录/settings.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub disabled_rules: Vec<String>,
}

/// 占用检测结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockerInfo {
    pub path: String,
    pub processes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesPayload {
    pub rules: Vec<Rule>,
    pub warnings: Vec<String>,
    pub user_rules_dir: String,
}
