//! 应用设置的持久化
//!
//! 有意不放进 SQLite ：设置项保存在数据目录下一个独立的 settings.json 里，
//! 方便用户直接导出/同步这一个文件，而不必操作整个任务数据库。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn default_trash_retention_days() -> u32 {
    30
}

fn default_font_size() -> u32 {
    14
}

fn default_duration_format() -> String {
    "%H:%M:%S".to_string()
}

fn default_due_time() -> String {
    "23:59".to_string()
}

fn default_true() -> bool {
    true
}

/// 图谱节点卡片详情行标签文字的最大字符数：卡片很窄，太长会被截断得很难看，
/// 需要和前端 SettingsModal.vue 里 `<input maxlength>` 保持一致
pub const NODE_LABEL_MAX_LEN: usize = 8;

// 图谱节点卡片详情行的默认标签文字，需要和前端 config/constants.js 里的
// DEFAULT_NODE_LABELS 保持一致（前端"重置"按钮显示的默认值就是这几个）
fn default_label_project() -> String {
    "项目".to_string()
}
fn default_label_due() -> String {
    "截止时间".to_string()
}
fn default_label_priority() -> String {
    "优先级".to_string()
}
fn default_label_recur() -> String {
    "重复".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// 废纸篓中的项目保留多少天后自动彻底删除；0 表示关闭自动清理
    #[serde(default = "default_trash_retention_days")]
    pub trash_retention_days: u32,
    /// 界面字体大小（像素）
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// 计时时长的显示格式，由前端 useDuration.js 解析
    /// （沿用 strftime 的 % 前缀记号：%D/%DD 天，%H/%h 时，%M/%m 分，%S/%s 秒）
    #[serde(default = "default_duration_format")]
    pub duration_format: String,
    /// 任务表单里只选日期、不显式选时间时，自动补上的默认到期时刻（"HH:MM"）
    #[serde(default = "default_due_time")]
    pub default_due_time: String,
    /// 以下四项控制图谱里任务节点卡片上默认显示哪些信息（悬浮详情窗不受影响，总是显示全部）
    #[serde(default = "default_true")]
    pub node_show_project: bool,
    #[serde(default = "default_true")]
    pub node_show_due: bool,
    #[serde(default = "default_true")]
    pub node_show_priority: bool,
    #[serde(default = "default_true")]
    pub node_show_recur: bool,
    /// 以下四项是上面对应信息在卡片上显示的标签文字，可以自定义
    #[serde(default = "default_label_project")]
    pub node_label_project: String,
    #[serde(default = "default_label_due")]
    pub node_label_due: String,
    #[serde(default = "default_label_priority")]
    pub node_label_priority: String,
    #[serde(default = "default_label_recur")]
    pub node_label_recur: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            trash_retention_days: default_trash_retention_days(),
            font_size: default_font_size(),
            duration_format: default_duration_format(),
            default_due_time: default_due_time(),
            node_show_project: default_true(),
            node_show_due: default_true(),
            node_show_priority: default_true(),
            node_show_recur: default_true(),
            node_label_project: default_label_project(),
            node_label_due: default_label_due(),
            node_label_priority: default_label_priority(),
            node_label_recur: default_label_recur(),
        }
    }
}

/// 校验 "HH:MM" 格式（00-23 时，00-59 分，两位数补零，对应 <input type="time"> 的输出格式）
pub fn validate_due_time(value: &str) -> bool {
    let Some((h, m)) = value.split_once(':') else {
        return false;
    };
    if h.len() != 2 || m.len() != 2 {
        return false;
    }
    matches!((h.parse::<u32>(), m.parse::<u32>()), (Ok(h), Ok(m)) if h < 24 && m < 60)
}

fn settings_path() -> std::path::PathBuf {
    crate::db::db_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
        .join("settings.json")
}

/// 读取设置文件；文件不存在或解析失败时返回默认值
pub fn load() -> Result<Settings> {
    let path = settings_path();

    if !path.exists() {
        return Ok(Settings::default());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("无法读取设置文件：{:?}", path))?;

    Ok(serde_json::from_str(&content).unwrap_or_default())
}

/// 写入设置文件
pub fn save(settings: &Settings) -> Result<()> {
    let path = settings_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建数据目录：{:?}", parent))?;
    }

    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content).with_context(|| format!("无法写入设置文件：{:?}", path))?;

    Ok(())
}
