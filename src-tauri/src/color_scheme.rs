//! 配色方案：内置预设 + 数据目录 themes/ 下的自定义 TOML 文件，两者走同一套
//! 结构体和校验逻辑，只是内容来源不同（见 get()）。只管颜色，不管字体/间距/
//! 圆角/材质——这些要么是全局固定的设计 token，要么是各自独立的设置项。

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    pub bg: Option<String>,
    #[serde(rename = "bg-dark")]
    pub bg_dark: Option<String>,
    #[serde(rename = "bg-panel")]
    pub bg_panel: Option<String>,
    #[serde(rename = "bg-select")]
    pub bg_select: Option<String>,
    #[serde(rename = "bg-popup")]
    pub bg_popup: Option<String>,
    pub fg: Option<String>,
    #[serde(rename = "fg-dim")]
    pub fg_dim: Option<String>,
    #[serde(rename = "fg-dark")]
    pub fg_dark: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub orange: Option<String>,
    pub red: Option<String>,
    pub border: Option<String>,
    #[serde(rename = "node-done")]
    pub node_done: Option<String>,
    #[serde(rename = "node-today")]
    pub node_today: Option<String>,
    #[serde(rename = "node-overdue")]
    pub node_overdue: Option<String>,
    #[serde(rename = "node-locked")]
    pub node_locked: Option<String>,
    #[serde(rename = "node-waiting")]
    pub node_waiting: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorScheme {
    #[serde(default)]
    pub name: Option<String>,
    pub light: Option<ColorTokens>,
    pub dark: Option<ColorTokens>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColorSchemeInfo {
    pub id: String,
    pub name: String,
}

/// 校验颜色值是否是合法的 `#rgb`/`#rrggbb`/`rgb(...)`/`rgba(...)`，
/// 不依赖 regex crate，手写小范围解析够用
pub fn is_valid_color(s: &str) -> bool {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        return parts.len() == 4 && parts.iter().all(|p| p.parse::<f64>().is_ok());
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        return parts.len() == 3 && parts.iter().all(|p| p.parse::<f64>().is_ok());
    }
    false
}

impl ColorTokens {
    /// 校验所有已设置的字段，返回第一个不合法字段的错误信息（带字段名，方便用户定位）
    pub fn validate(&self) -> Result<(), String> {
        let fields: [(&str, &Option<String>); 21] = [
            ("bg", &self.bg),
            ("bg-dark", &self.bg_dark),
            ("bg-panel", &self.bg_panel),
            ("bg-select", &self.bg_select),
            ("bg-popup", &self.bg_popup),
            ("fg", &self.fg),
            ("fg-dim", &self.fg_dim),
            ("fg-dark", &self.fg_dark),
            ("blue", &self.blue),
            ("magenta", &self.magenta),
            ("cyan", &self.cyan),
            ("green", &self.green),
            ("yellow", &self.yellow),
            ("orange", &self.orange),
            ("red", &self.red),
            ("border", &self.border),
            ("node-done", &self.node_done),
            ("node-today", &self.node_today),
            ("node-overdue", &self.node_overdue),
            ("node-locked", &self.node_locked),
            ("node-waiting", &self.node_waiting),
        ];
        for (name, value) in fields {
            if let Some(v) = value {
                if !is_valid_color(v) {
                    return Err(format!(
                        "配色方案里 \"{}\" 的值 \"{}\" 不是合法的颜色格式",
                        name, v
                    ));
                }
            }
        }
        Ok(())
    }
}

/// 数据目录下的自定义配色方案文件夹，跟 settings.json/tasks.db 同级
fn themes_dir() -> std::path::PathBuf {
    crate::db::db_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
        .join("themes")
}

fn builtin_content(name: &str) -> Option<&'static str> {
    match name {
        "b" => Some(include_str!("themes/b.toml")),
        "c" => Some(include_str!("themes/c.toml")),
        "d" => Some(include_str!("themes/d.toml")),
        _ => None,
    }
}

/// 列出所有可选配色方案：3 个内置 + 数据目录 themes/ 下发现的 .toml 文件，
/// 每项带一个可直接显示的名字（自定义文件读它自己的 name 字段，没写就用文件名兜底）
pub fn list_all() -> Vec<ColorSchemeInfo> {
    let mut result = Vec::new();
    for key in ["b", "c", "d"] {
        if let Some(content) = builtin_content(key) {
            if let Ok(scheme) = toml::from_str::<ColorScheme>(content) {
                let name = scheme.name.unwrap_or_else(|| format!("内置方案 {}", key));
                result.push(ColorSchemeInfo {
                    id: format!("builtin:{}", key),
                    name,
                });
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(themes_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let name = std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| toml::from_str::<ColorScheme>(&content).ok())
                .and_then(|scheme| scheme.name)
                .unwrap_or_else(|| stem.to_string());
            result.push(ColorSchemeInfo {
                id: format!("custom:{}", stem),
                name,
            });
        }
    }
    result
}

/// 按 id 加载一个配色方案（内置走 include_str! 嵌入内容，自定义走磁盘读取），
/// 解析并校验后返回
pub fn get(id: &str) -> Result<ColorScheme, String> {
    let content = if let Some(key) = id.strip_prefix("builtin:") {
        builtin_content(key)
            .ok_or_else(|| format!("未知的内置配色方案：{}", key))?
            .to_string()
    } else if let Some(stem) = id.strip_prefix("custom:") {
        if stem.is_empty() || stem.contains('/') || stem.contains('\\') || stem.contains("..") {
            return Err("非法的自定义配色文件名".to_string());
        }
        let path = themes_dir().join(format!("{}.toml", stem));
        std::fs::read_to_string(&path).map_err(|e| format!("读取自定义配色文件失败：{}", e))?
    } else {
        return Err(format!("未知的配色方案 id：{}", id));
    };

    let scheme: ColorScheme =
        toml::from_str(&content).map_err(|e| format!("配色文件解析失败：{}", e))?;
    if let Some(light) = &scheme.light {
        light.validate()?;
    }
    if let Some(dark) = &scheme.dark {
        dark.validate()?;
    }
    Ok(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_color_accepts_known_forms() {
        assert!(is_valid_color("#fff"));
        assert!(is_valid_color("#4f6bff"));
        assert!(is_valid_color("rgb(79, 107, 255)"));
        assert!(is_valid_color("rgba(79, 107, 255, 0.18)"));
    }

    #[test]
    fn is_valid_color_rejects_bad_forms() {
        assert!(!is_valid_color("blue"));
        assert!(!is_valid_color("#ff"));
        assert!(!is_valid_color("#gggggg"));
        assert!(!is_valid_color("rgba(1,2,3)"));
        assert!(!is_valid_color(""));
    }

    #[test]
    fn color_tokens_validate_passes_with_partial_valid_values() {
        let tokens = ColorTokens {
            blue: Some("#4f6bff".to_string()),
            node_done: Some("rgba(63,174,106,.18)".to_string()),
            ..Default::default()
        };
        assert!(tokens.validate().is_ok());
    }

    #[test]
    fn color_tokens_validate_fails_and_names_the_field() {
        let tokens = ColorTokens {
            blue: Some("not-a-color".to_string()),
            ..Default::default()
        };
        let err = tokens.validate().unwrap_err();
        assert!(err.contains("blue"));
    }

    #[test]
    fn deserializing_unknown_field_is_rejected() {
        let toml_str = r##"
            [light]
            blue = "#4f6bff"
            not-a-real-token = "#000000"
        "##;
        let result: Result<ColorScheme, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn deserializing_missing_keys_leaves_them_none() {
        let toml_str = r##"
            name = "测试方案"
            [light]
            blue = "#4f6bff"
        "##;
        let scheme: ColorScheme = toml::from_str(toml_str).unwrap();
        assert_eq!(scheme.name.as_deref(), Some("测试方案"));
        let light = scheme.light.unwrap();
        assert_eq!(light.blue.as_deref(), Some("#4f6bff"));
        assert!(light.bg.is_none());
        assert!(scheme.dark.is_none());
    }

    #[test]
    fn get_builtin_b_parses_and_validates() {
        let scheme = get("builtin:b").expect("内置方案 b 应该能正常加载");
        assert_eq!(scheme.name.as_deref(), Some("沉稳深色系统"));
        assert!(scheme.light.is_some());
        assert!(scheme.dark.is_some());
    }

    #[test]
    fn get_builtin_c_and_d_parse() {
        assert!(get("builtin:c").is_ok());
        assert!(get("builtin:d").is_ok());
    }

    #[test]
    fn get_unknown_builtin_fails() {
        assert!(get("builtin:a").is_err());
    }

    #[test]
    fn get_custom_rejects_path_traversal() {
        assert!(get("custom:../../etc/passwd").is_err());
        assert!(get("custom:sub/dir").is_err());
    }

    #[test]
    fn get_unknown_prefix_fails() {
        assert!(get("nonsense").is_err());
    }
}
