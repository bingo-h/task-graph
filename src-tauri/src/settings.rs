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

fn default_font_family() -> String {
    "sans-serif".to_string()
}

fn default_duration_format() -> String {
    "%H:%M:%S".to_string()
}

fn default_due_time() -> String {
    "23:59".to_string()
}

fn default_inbox_label() -> String {
    "无项目".to_string()
}

fn default_notification_duration_seconds() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

fn default_color_scheme() -> String {
    String::new()
}

fn default_theme_mode() -> String {
    "light".to_string()
}

fn default_corner_radius() -> u32 {
    10
}

fn default_ui_style() -> String {
    "flat".to_string()
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
    /// 界面字体家族名（从系统已安装字体里选，前端直接用作 CSS font-family）
    #[serde(default = "default_font_family")]
    pub font_family: String,
    /// 图谱任务节点卡片单独的字体；空字符串表示跟随上面的 font_family
    #[serde(default)]
    pub node_font_family: String,
    /// 计时时长的显示格式，由前端 useDuration.js 解析
    /// （沿用 strftime 的 % 前缀记号：%D/%DD 天，%H/%h 时，%M/%m 分，%S/%s 秒）
    #[serde(default = "default_duration_format")]
    pub duration_format: String,
    /// 任务表单里只选日期、不显式选时间时，自动补上的默认到期时刻（"HH:MM"）
    #[serde(default = "default_due_time")]
    pub default_due_time: String,
    /// "无项目"虚拟归集节点在界面上显示的名字；只是显示文字，内部仍用固定的
    /// "无项目"作为筛选/匹配用的哨兵值，不受这个设置影响
    #[serde(default = "default_inbox_label")]
    pub inbox_label: String,
    /// 错误提示悬浮通知自动消失前停留的时间（秒）
    #[serde(default = "default_notification_duration_seconds")]
    pub notification_duration_seconds: u32,
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
    /// 配色方案："" 表示内置默认（方案 A）；"builtin:b"/"builtin:c"/"builtin:d" 表示内置预设；
    /// "custom:<文件名，不含扩展名>" 表示数据目录 themes/ 下的自定义文件
    #[serde(default)]
    pub color_scheme: String,
    /// 深浅模式："system" | "light" | "dark"
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    /// 全局圆角基准（像素），前端据此派生 --radius-sm/md/lg 三档
    #[serde(default = "default_corner_radius")]
    pub corner_radius: u32,
    /// 界面风格（只影响顶栏/侧栏/右键菜单/统计卡片等外壳，不影响 DAG 图任务节点）：
    /// "flat" | "glass" | "neumorphism"
    #[serde(default = "default_ui_style")]
    pub ui_style: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            trash_retention_days: default_trash_retention_days(),
            font_size: default_font_size(),
            font_family: default_font_family(),
            node_font_family: String::new(),
            duration_format: default_duration_format(),
            default_due_time: default_due_time(),
            inbox_label: default_inbox_label(),
            notification_duration_seconds: default_notification_duration_seconds(),
            node_show_project: default_true(),
            node_show_due: default_true(),
            node_show_priority: default_true(),
            node_show_recur: default_true(),
            node_label_project: default_label_project(),
            node_label_due: default_label_due(),
            node_label_priority: default_label_priority(),
            node_label_recur: default_label_recur(),
            color_scheme: default_color_scheme(),
            theme_mode: default_theme_mode(),
            corner_radius: default_corner_radius(),
            ui_style: default_ui_style(),
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

/// 校验深浅模式取值
pub fn validate_theme_mode(value: &str) -> bool {
    matches!(value, "system" | "light" | "dark")
}

/// 校验界面风格取值
pub fn validate_ui_style(value: &str) -> bool {
    matches!(value, "flat" | "glass" | "neumorphism")
}

/// 校验配色方案 id 的格式（不校验方案本身是否存在/合法，那是 color_scheme::get 的职责）：
/// 允许空串（默认），或 "builtin:b"/"builtin:c"/"builtin:d"，或 "custom:<不含路径分隔符的文件名>"
pub fn validate_color_scheme_id(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if matches!(value, "builtin:b" | "builtin:c" | "builtin:d") {
        return true;
    }
    if let Some(name) = value.strip_prefix("custom:") {
        return !name.is_empty()
            && !name.contains('/')
            && !name.contains('\\')
            && !name.contains("..");
    }
    false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_accepts_known_values() {
        assert!(validate_theme_mode("system"));
        assert!(validate_theme_mode("light"));
        assert!(validate_theme_mode("dark"));
        assert!(!validate_theme_mode("auto"));
    }

    #[test]
    fn ui_style_accepts_known_values() {
        assert!(validate_ui_style("flat"));
        assert!(validate_ui_style("glass"));
        assert!(validate_ui_style("neumorphism"));
        assert!(!validate_ui_style("skeuomorphic"));
    }

    #[test]
    fn color_scheme_id_accepts_valid_forms() {
        assert!(validate_color_scheme_id(""));
        assert!(validate_color_scheme_id("builtin:b"));
        assert!(validate_color_scheme_id("builtin:c"));
        assert!(validate_color_scheme_id("builtin:d"));
        assert!(validate_color_scheme_id("custom:my-theme"));
    }

    #[test]
    fn color_scheme_id_rejects_bad_forms() {
        assert!(!validate_color_scheme_id("builtin:a"));
        assert!(!validate_color_scheme_id("custom:"));
        assert!(!validate_color_scheme_id("custom:../../etc/passwd"));
        assert!(!validate_color_scheme_id("custom:sub/dir"));
        assert!(!validate_color_scheme_id("random"));
    }
}
