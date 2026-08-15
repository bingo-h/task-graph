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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            trash_retention_days: default_trash_retention_days(),
            font_size: default_font_size(),
            duration_format: default_duration_format(),
        }
    }
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
