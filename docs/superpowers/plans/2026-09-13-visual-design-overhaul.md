# 视觉设计体系重做 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 task-graph 加上可切换的配色方案（内置预设 + 自定义 TOML 文件）、深浅模式、圆角设置、三选一的界面风格（扁平/液态玻璃/新拟态，只影响外壳），并把间距/阴影/动效/按钮/分段控件收敛成统一的 token 体系。

**Architecture:** 后端新增 `color_scheme` 模块（TOML 解析+校验，内置预设用 `include_str!` 嵌入、自定义预设从数据目录 `themes/` 读取，两者走同一套代码返回给前端）；`Settings` 结构体新增四个展示层字段（不影响 `build_graph()`）。前端全部改动是 CSS 自定义属性 + 少量 JS 应用逻辑：颜色/圆角/风格切换都是往 `<html>` 上设置属性或 CSS 变量，组件样式只负责"读 token"，不用 JS 挨个操作每个组件。

**Tech Stack:** Rust（Tauri command、`toml` crate）、Vue 3 `<script setup>`、原生 CSS 自定义属性（无 CSS 预处理器）。

**Spec:** `docs/superpowers/specs/2026-09-13-visual-design-overhaul-design.md`

## Global Constraints

- 所有注释、commit message 用简体中文；代码本身的标识符（变量名/函数名/CSS 类名）用英文。
- 新增依赖一律用命令行工具添加（`cargo add <crate>` / `pnpm add <pkg>`），不手动改 `Cargo.toml`/`package.json`；不自己指定版本号，让工具解析最新版本，只有装不上/编译报不兼容错误时才根据报错降级到具体版本。
- 整个功能落地后在 `CHANGELOG.md`（没有就新建，含"未发布"章节）统一补一次记录，具体到文件/函数/设置项级别——本仓库的习惯是把功能提交和变更日志提交分开（参考 `git log` 里 `db08615 chore: 更新变更日志` 这类独立的日志提交，而不是每个小提交都各自维护一条），这次统一放在 Task 18 做，Task 1-17 不用各自更新 CHANGELOG。
- 新 Tauri 命令的 JS 调用参数用 camelCase（除非命令签名是单个 struct 参数，此时该 struct 内部字段保持 snake_case）。
- `list_color_schemes`/`get_color_scheme` 这两个新命令不调用 `build_graph()`——纯展示层读取，套用 `get_settings`/`list_system_fonts` 的模式，不是"改数据"命令的 `Args → db:: → build_graph()` 模式。
- 前端没有测试框架和 lint 配置，"测试"步骤统一是 `cd frontend && pnpm run build` 跑通；不要发明 `pnpm test`/`pnpm run lint`。
- Rust 测试用项目已有的内联约定：`#[cfg(test)] mod tests { use super::*; ... }` 写在被测代码所在文件末尾（参考 `src-tauri/src/graph_utils.rs`），不用单独的 `tests/` 目录。

---

## Task 1: Settings 新增四个字段（后端）

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/commands.rs:521-565`（`save_settings` 校验分支）

**Interfaces:**
- Produces: `Settings.color_scheme: String`、`Settings.theme_mode: String`、`Settings.corner_radius: u32`、`Settings.ui_style: String`，以及 `default_color_scheme()`/`default_theme_mode()`/`default_corner_radius()`/`default_ui_style()` 四个默认值函数——后面所有任务读写这四个字段都用这些名字。

- [ ] **Step 1: 在 `settings.rs` 加默认值函数和字段**

在 `src-tauri/src/settings.rs` 里 `fn default_true()` 之后加：

```rust
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
```

在 `Settings` 结构体的 `node_label_recur: String,` 字段之后加：

```rust
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
    /// 界面风格（只影响顶栏/侧栏/弹窗/下拉菜单/统计卡片等外壳，不影响 DAG 图任务节点）：
    /// "flat" | "glass" | "neumorphism"
    #[serde(default = "default_ui_style")]
    pub ui_style: String,
```

在 `impl Default for Settings` 的 `Settings { ... }` 字面量里，`node_label_recur: default_label_recur(),` 之后加：

```rust
            color_scheme: default_color_scheme(),
            theme_mode: default_theme_mode(),
            corner_radius: default_corner_radius(),
            ui_style: default_ui_style(),
```

- [ ] **Step 2: 加校验函数并写失败测试**

在 `settings.rs` 的 `validate_due_time` 函数之后加两个校验函数：

```rust
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
```

在文件末尾加测试模块：

```rust
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
```

- [ ] **Step 3: 运行测试确认通过**

Run: `cd src-tauri && cargo test settings::tests`
Expected: 4 个测试全部 PASS

- [ ] **Step 4: `save_settings` 加校验分支**

在 `src-tauri/src/commands.rs` 的 `save_settings` 函数里，找到已有的最后一个校验块（`for label in [...] { ... }` 之后、`db::settings...` 或函数返回之前，即原先的最后一段校验逻辑之后），追加：

```rust
    if !crate::settings::validate_theme_mode(&settings.theme_mode) {
        return Err("深浅模式取值不合法".to_string());
    }
    if settings.corner_radius > 24 {
        return Err("圆角数值超出范围（0-24）".to_string());
    }
    if !crate::settings::validate_ui_style(&settings.ui_style) {
        return Err("界面风格取值不合法".to_string());
    }
    if !crate::settings::validate_color_scheme_id(&settings.color_scheme) {
        return Err("配色方案 id 格式不合法".to_string());
    }
```

- [ ] **Step 5: `cargo check` 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 无报错（`save_settings` 原有函数体后半段应该是 `crate::settings::save(&settings)...` 或类似收尾，新校验分支插入位置只要在参数校验阶段、写盘之前即可，不影响收尾逻辑）

- [ ] **Step 6: Commit**

```bash
cd src-tauri && git add src/settings.rs src/commands.rs && git commit -m "$(cat <<'EOF'
feat: Settings 新增配色方案/深浅模式/圆角/界面风格四个字段

新增 color_scheme/theme_mode/corner_radius/ui_style 字段及默认值、
校验函数，save_settings 接入校验。这四项都是纯展示层设置，不影响
build_graph()。
EOF
)"
```

---

## Task 2: 配色方案数据结构与校验（`color_scheme.rs` 模块）

**Files:**
- Create: `src-tauri/src/color_scheme.rs`
- Modify: `src-tauri/src/lib.rs:7-12`（加 `mod color_scheme;`）

**Interfaces:**
- Consumes: 无（纯新模块，不依赖 Task 1）
- Produces: `ColorTokens`（21 个 `Option<String>` 字段的结构体）、`ColorScheme { name: Option<String>, light: Option<ColorTokens>, dark: Option<ColorTokens> }`、`ColorSchemeInfo { id: String, name: String }`、`is_valid_color(&str) -> bool`、`ColorTokens::validate(&self) -> Result<(), String>`——Task 3/4 都要用这些类型和函数名。

- [ ] **Step 1: 用命令行加 `toml` 依赖**

Run: `cd src-tauri && cargo add toml`
Expected: `Cargo.toml` 的 `[dependencies]` 里出现一行 `toml = "<最新版本号>"`（版本号由 cargo 自动写入，不要手动改成别的版本）

- [ ] **Step 2: 写失败的颜色校验测试**

创建 `src-tauri/src/color_scheme.rs`：

```rust
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
        let toml_str = r#"
            [light]
            blue = "#4f6bff"
            not-a-real-token = "#000000"
        "#;
        let result: Result<ColorScheme, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn deserializing_missing_keys_leaves_them_none() {
        let toml_str = r#"
            name = "测试方案"
            [light]
            blue = "#4f6bff"
        "#;
        let scheme: ColorScheme = toml::from_str(toml_str).unwrap();
        assert_eq!(scheme.name.as_deref(), Some("测试方案"));
        let light = scheme.light.unwrap();
        assert_eq!(light.blue.as_deref(), Some("#4f6bff"));
        assert!(light.bg.is_none());
        assert!(scheme.dark.is_none());
    }
}
```

- [ ] **Step 3: 运行测试确认通过**

Run: `cd src-tauri && cargo test color_scheme::tests`
Expected: 6 个测试全部 PASS

- [ ] **Step 4: 在 `lib.rs` 注册新模块**

在 `src-tauri/src/lib.rs` 里 `mod commands;` 之后加一行：

```rust
mod color_scheme;
```

- [ ] **Step 5: `cargo check` 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 无报错（此时 `color_scheme` 模块还没被其它模块使用，会有"从未使用"的 warning，属于预期，Task 4 接入命令后会消失）

- [ ] **Step 6: Commit**

```bash
cd src-tauri && git add Cargo.toml Cargo.lock src/lib.rs src/color_scheme.rs && git commit -m "$(cat <<'EOF'
feat: 新增配色方案的数据结构、颜色值校验和 TOML 解析

ColorTokens（21 个可选颜色字段）+ ColorScheme（light/dark 两组）+
is_valid_color 手写校验（不依赖 regex）。deny_unknown_fields 保证
未知 key 在反序列化阶段就报错。
EOF
)"
```

---

## Task 3: 内置配色预设文件（B/C/D）与加载逻辑

**Files:**
- Create: `src-tauri/src/themes/b.toml`
- Create: `src-tauri/src/themes/c.toml`
- Create: `src-tauri/src/themes/d.toml`
- Modify: `src-tauri/src/color_scheme.rs`（加 `themes_dir()`/`list_all()`/`get()`）

**Interfaces:**
- Consumes: Task 2 的 `ColorTokens`/`ColorScheme`/`ColorSchemeInfo`
- Produces: `color_scheme::themes_dir() -> PathBuf`、`color_scheme::list_all() -> Vec<ColorSchemeInfo>`、`color_scheme::get(id: &str) -> Result<ColorScheme, String>`——Task 4 的 Tauri 命令直接调用这两个函数。

- [ ] **Step 1: 写三个内置预设 TOML 文件**

所有方案的 `green`/`red` 在同一深浅模式下取值一致（成功/危险是接近普适的语义色，不随方案"调性"漂移），只有中性色（bg/fg/border）和 blue/magenta/cyan/yellow/orange 这几个"个性色相"随方案变化。

创建 `src-tauri/src/themes/b.toml`（沉稳深色系统，暗色为主）：

```toml
name = "沉稳深色系统"

[light]
bg = "#f7f7f8"
bg-dark = "#ffffff"
bg-panel = "#ffffff"
bg-select = "#ece7fb"
bg-popup = "#ffffff"
fg = "#16171a"
fg-dim = "#6f707a"
fg-dark = "#a9aab3"
border = "#e2e2e6"
blue = "#6a4fe0"
magenta = "#8a3f96"
cyan = "#2c7f96"
green = "#1a7f37"
yellow = "#9a6700"
orange = "#bc4c00"
red = "#d1242f"
node-done = "#e4f5ea"
node-today = "#ece7fb"
node-overdue = "#fbe4e7"
node-locked = "#eceef1"
node-waiting = "#eceef4"

[dark]
bg = "#101114"
bg-dark = "#17181c"
bg-panel = "#17181c"
bg-select = "rgba(143,125,255,.18)"
bg-popup = "#17181c"
fg = "#f2f2f3"
fg-dim = "#9a9aa2"
fg-dark = "#55565f"
border = "#26272c"
blue = "#8f7dff"
magenta = "#c77fd6"
cyan = "#5fb8d6"
green = "#3fae6a"
yellow = "#d9a72e"
orange = "#e0904f"
red = "#f2757a"
node-done = "rgba(63,174,106,.18)"
node-today = "rgba(143,125,255,.22)"
node-overdue = "rgba(255,107,122,.2)"
node-locked = "rgba(255,255,255,.06)"
node-waiting = "rgba(255,255,255,.04)"
```

创建 `src-tauri/src/themes/c.toml`（柔和明快，浅色为主）：

```toml
name = "柔和明快"

[light]
bg = "#faf8f5"
bg-dark = "#fffdfb"
bg-panel = "#fffdfb"
bg-select = "#ffe3d6"
bg-popup = "#fffdfb"
fg = "#33302b"
fg-dim = "#8a8378"
fg-dark = "#c9c0b2"
border = "#ece5da"
blue = "#ff8a65"
magenta = "#b39ddb"
cyan = "#5aa8a0"
green = "#1a7f37"
yellow = "#c98a2e"
orange = "#e0654a"
red = "#d1242f"
node-done = "#dcf1e4"
node-today = "#efe4fb"
node-overdue = "#ffe0d3"
node-locked = "#efe8dd"
node-waiting = "#f2ece0"

[dark]
bg = "#241f1a"
bg-dark = "#2c2620"
bg-panel = "#2c2620"
bg-select = "rgba(255,171,138,.2)"
bg-popup = "#2c2620"
fg = "#f2e9de"
fg-dim = "#b7a996"
fg-dark = "#5c5346"
border = "#3a332b"
blue = "#ffab8a"
magenta = "#c7a8e0"
cyan = "#7bc4bc"
green = "#3fae6a"
yellow = "#e0a94f"
orange = "#ff8a65"
red = "#f2757a"
node-done = "rgba(120,200,150,.18)"
node-today = "rgba(190,150,230,.2)"
node-overdue = "rgba(255,138,101,.22)"
node-locked = "rgba(255,255,255,.06)"
node-waiting = "rgba(255,255,255,.05)"
```

创建 `src-tauri/src/themes/d.toml`（极简黑白灰，浅色为主）：

```toml
name = "极简黑白灰"

[light]
bg = "#ffffff"
bg-dark = "#ffffff"
bg-panel = "#ffffff"
bg-select = "#e7f0fb"
bg-popup = "#ffffff"
fg = "#37352f"
fg-dim = "#9b9a97"
fg-dark = "#cfceca"
border = "#e9e9e7"
blue = "#2383e2"
magenta = "#8a7fb0"
cyan = "#4592a8"
green = "#1a7f37"
yellow = "#9a6700"
orange = "#bc4c00"
red = "#d1242f"
node-done = "#eef2ea"
node-today = "#f6f1e6"
node-overdue = "#faeae8"
node-locked = "#f2f2f1"
node-waiting = "#f2f2f0"

[dark]
bg = "#191919"
bg-dark = "#202020"
bg-panel = "#202020"
bg-select = "rgba(95,168,245,.15)"
bg-popup = "#202020"
fg = "#e9e9e7"
fg-dim = "#9b9a97"
fg-dark = "#4a4a48"
border = "#2f2f2f"
blue = "#5fa8f5"
magenta = "#a89bc7"
cyan = "#5fa8ad"
green = "#3fae6a"
yellow = "#d9a72e"
orange = "#e0904f"
red = "#f2757a"
node-done = "rgba(120,150,110,.15)"
node-today = "rgba(220,190,120,.15)"
node-overdue = "rgba(224,100,86,.15)"
node-locked = "rgba(255,255,255,.05)"
node-waiting = "rgba(255,255,255,.04)"
```

- [ ] **Step 2: 写会失败的加载测试**

在 `src-tauri/src/color_scheme.rs` 末尾的 `mod tests` 里加：

```rust
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
```

- [ ] **Step 3: 运行测试确认失败（`get`/`themes_dir` 还不存在）**

Run: `cd src-tauri && cargo test color_scheme::tests`
Expected: FAIL，报 `cannot find function 'get' in this scope`

- [ ] **Step 4: 实现 `themes_dir`/`list_all`/`get`**

在 `src-tauri/src/color_scheme.rs` 里、`impl ColorTokens` 代码块之后加：

```rust
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
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test color_scheme::tests`
Expected: 全部 PASS（共 11 个测试：Task 2 的 6 个 + 这一步的 5 个）

- [ ] **Step 6: Commit**

```bash
cd src-tauri && git add src/themes/ src/color_scheme.rs && git commit -m "$(cat <<'EOF'
feat: 新增内置配色预设 B/C/D 及加载逻辑

三套内置预设用 include_str! 嵌入二进制；自定义预设从数据目录
themes/ 读取；两者统一走 color_scheme::get()，返回前统一做
deny_unknown_fields + 颜色值格式校验。green/red 在所有方案里保持
一致（成功/危险语义不随方案调性变化）。
EOF
)"
```

---

## Task 4: Tauri 命令与注册

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs:38-78`

**Interfaces:**
- Consumes: Task 3 的 `color_scheme::list_all()`/`color_scheme::get()`
- Produces: Tauri 命令 `list_color_schemes`、`get_color_scheme`——Task 9 的前端 `useApi.js` 封装直接调这两个命令名。

- [ ] **Step 1: 加命令函数**

在 `src-tauri/src/commands.rs` 里 `list_system_fonts` 函数之后加：

```rust
/// 列出所有可选配色方案（内置 3 套 + 数据目录 themes/ 下发现的自定义文件）
#[tauri::command]
pub fn list_color_schemes() -> Vec<crate::color_scheme::ColorSchemeInfo> {
    crate::color_scheme::list_all()
}

/// 按 id 加载一个配色方案，校验后返回
#[tauri::command]
pub fn get_color_scheme(id: String) -> Result<crate::color_scheme::ColorScheme, String> {
    crate::color_scheme::get(&id)
}
```

- [ ] **Step 2: 在 `lib.rs` 注册命令**

在 `src-tauri/src/lib.rs` 的 `invoke_handler(tauri::generate_handler![...])` 列表里，`commands::list_system_fonts,` 之后加：

```rust
            commands::list_color_schemes,
            commands::get_color_scheme,
```

- [ ] **Step 3: `cargo check` 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 无报错

- [ ] **Step 4: 跑一遍全部后端测试确认没有破坏其它模块**

Run: `cd src-tauri && cargo test`
Expected: 全部 PASS（包括这次新增的 15 个测试和项目原有的 schema/recur/graph_utils 测试）

- [ ] **Step 5: Commit**

```bash
cd src-tauri && git add src/commands.rs src/lib.rs && git commit -m "$(cat <<'EOF'
feat: 新增 list_color_schemes/get_color_scheme Tauri 命令

薄封装，直接委托给 color_scheme 模块；不调用 build_graph()，
跟 get_settings/list_system_fonts 是同一类纯展示层读取命令。
EOF
)"
```

---

## Task 5: `style.css` 配色 token 重写（方案 A 默认值 + 深色块 + 节点状态色变量化）

**Files:**
- Modify: `frontend/src/style.css`

**Interfaces:**
- Produces: `:root` 里 21 个颜色 token（跟 Task 2 的 `ColorTokens` 字段一一对应）+ `:root[data-theme-mode="dark"]` 覆盖块；新增 `--node-done`/`--node-today`/`--node-overdue`/`--node-locked`/`--node-waiting` 五个变量，供后面 `.rect-*` 规则和 Task 8 的配色应用逻辑使用。

- [ ] **Step 1: 替换 `:root` 块为方案 A 的浅色默认值**

把 `frontend/src/style.css` 开头的：

```css
/* 亮色配色 */
:root {
    --bg: #f5f6f8;
    --bg-dark: #ffffff;
    --bg-panel: #ffffff;
    --bg-select: #e3ecfb;
    --bg-popup: #ffffff;
    --fg: #24292f;
    --fg-dim: #6e7781;
    --fg-dark: #afb6c0;
    --blue: #1a73e8;
    --magenta: #8250df;
    --cyan: #0598bc;
    --green: #1a7f37;
    --yellow: #9a6700;
    --orange: #bc4c00;
    --red: #d1242f;
    --border: #d0d7de;
}
```

整段换成：

```css
/* 默认配色方案 A（克制中性 + 单一强调色），浅色 */
:root {
    --bg: #fbfbfd;
    --bg-dark: #ffffff;
    --bg-panel: #ffffff;
    --bg-select: #e8ecff;
    --bg-popup: #ffffff;
    --fg: #1c1f26;
    --fg-dim: #6b7280;
    --fg-dark: #9aa1ad;
    --blue: #4f6bff;
    --magenta: #8250df;
    --cyan: #0598bc;
    --green: #1a7f37;
    --yellow: #9a6700;
    --orange: #bc4c00;
    --red: #d1242f;
    --border: #e4e4ea;
    --node-done: #e6f5ec;
    --node-today: #fdf1dc;
    --node-overdue: #fdeceb;
    --node-locked: #eceef2;
    --node-waiting: #e9edf5;
}

/* 同一份方案 A 的深色默认值。不用 @media (prefers-color-scheme) 是因为
   "该用浅色还是深色"由 App.vue 的 theme_mode 设置解析后写成这个属性，
   不依赖操作系统主题查询——这样"跟随系统"和"手动选浅/深"能走同一套 CSS。 */
:root[data-theme-mode="dark"] {
    --bg: #0d0f14;
    --bg-dark: #14161d;
    --bg-panel: #14161d;
    --bg-select: rgba(124, 143, 255, 0.18);
    --bg-popup: #14161d;
    --fg: #e6e8ef;
    --fg-dim: #8b8fa3;
    --fg-dark: #4b5165;
    --blue: #7c8fff;
    --magenta: #b48eea;
    --cyan: #4dc3e0;
    --green: #3fae6a;
    --yellow: #d9a72e;
    --orange: #e0904f;
    --red: #f2757a;
    --border: #262933;
    --node-done: rgba(63, 174, 106, 0.18);
    --node-today: rgba(217, 167, 46, 0.18);
    --node-overdue: rgba(242, 117, 122, 0.18);
    --node-locked: rgba(255, 255, 255, 0.07);
    --node-waiting: rgba(255, 255, 255, 0.05);
}
```

- [ ] **Step 2: 把 `.rect-*` 硬编码颜色改成引用新变量**

把：

```css
.rect-done {
    fill: #e6f4ea;
}

.rect-overdue {
    fill: rgba(247, 118, 142, 0.18);
}
```

```css
.rect-today {
    fill: #fdf0dd;
}

.rect-locked {
    fill: #eceef1;
}

.rect-waiting {
    fill: #e9edf5;
}
```

分别改成：

```css
.rect-done {
    fill: var(--node-done);
}

.rect-overdue {
    fill: var(--node-overdue);
}
```

```css
.rect-today {
    fill: var(--node-today);
}

.rect-locked {
    fill: var(--node-locked);
}

.rect-waiting {
    fill: var(--node-waiting);
}
```

- [ ] **Step 3: `pnpm run build` 确认没有语法错误**

Run: `cd frontend && pnpm run build`
Expected: 构建成功，无报错

- [ ] **Step 4: Commit**

```bash
git add frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: style.css 配色改用方案 A 默认值，新增深色块和节点状态色变量

:root 换成方案 A 的浅色 token；新增 :root[data-theme-mode="dark"]
深色块；.rect-done/.rect-today/.rect-overdue/.rect-locked/
.rect-waiting 的硬编码颜色提取成 --node-* 变量，使其能被配色方案
覆盖。
EOF
)"
```

---

## Task 6: `style.css` 圆角/间距/阴影/动效 token

**Files:**
- Modify: `frontend/src/style.css`

**Interfaces:**
- Consumes: 无
- Produces: `--app-radius`/`--radius-sm`/`--radius-md`/`--radius-lg`、`--space-1`~`--space-8`、`--elevation-1`/`--elevation-2`/`--elevation-3`（浅深各一套）、`--ease-standard`/`--ease-spring`——后面所有组件级任务（12-17）都消费这些 token。

- [ ] **Step 1: 在 `:root` 块末尾（`--node-waiting` 之后）加圆角/间距/动效 token**

```css
    /* 圆角：用户可调的基准值，派生三档 */
    --app-radius: 10px;
    --radius-sm: calc(var(--app-radius) * 0.6);
    --radius-md: var(--app-radius);
    --radius-lg: calc(var(--app-radius) * 1.4);

    /* 间距：4px 基准网格 */
    --space-1: 4px;
    --space-2: 8px;
    --space-3: 12px;
    --space-4: 16px;
    --space-5: 20px;
    --space-6: 24px;
    --space-7: 28px;
    --space-8: 32px;

    /* 阴影分级：静止面板 / 悬浮态 / 弹出层-Modal */
    --elevation-1: 0 1px 2px rgba(16, 24, 40, 0.04), 0 1px 3px rgba(16, 24, 40, 0.06);
    --elevation-2: 0 4px 14px rgba(16, 24, 40, 0.1), 0 2px 4px rgba(16, 24, 40, 0.04);
    --elevation-3: 0 24px 48px rgba(16, 24, 40, 0.18), 0 8px 20px rgba(16, 24, 40, 0.08);

    /* 缓动曲线：常规交互 / 弹窗与悬浮层出现 */
    --ease-standard: cubic-bezier(0.4, 0, 0.2, 1);
    --ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1);
```

在 `:root[data-theme-mode="dark"]` 块末尾（`--node-waiting` 之后）加深色版的阴影（黑色阴影在深色背景上不够用，改用"顶部内嵌高光 + 更重的黑色投影"）：

```css
    --elevation-1: 0 1px 0 rgba(255, 255, 255, 0.03) inset, 0 2px 8px rgba(0, 0, 0, 0.4);
    --elevation-2: 0 1px 0 rgba(255, 255, 255, 0.04) inset, 0 10px 24px rgba(0, 0, 0, 0.5);
    --elevation-3: 0 1px 0 rgba(255, 255, 255, 0.05) inset, 0 28px 60px rgba(0, 0, 0, 0.6);
```

- [ ] **Step 2: 把 D3 节点矩形接入圆角 token**

在 `.node-desc` 规则之前加一条新规则（节点矩形目前没有设置 `rx`，这是新增）：

```css
.node rect {
    rx: var(--radius-sm);
}
```

- [ ] **Step 3: 把已有的过渡动效改用新的缓动曲线**

把 `.node rect` 规则里的：

```css
    transition:
        opacity 0.25s,
        filter 0.25s,
        stroke 0.25s,
        stroke-width 0.25s;
```

改成：

```css
    transition:
        opacity 0.25s var(--ease-standard),
        filter 0.25s var(--ease-standard),
        stroke 0.25s var(--ease-standard),
        stroke-width 0.25s var(--ease-standard);
```

同样的替换应用到 `.connect-dot`、`.edge`、`.edge-handle-dot` 这三处已有的 `transition` 声明——每个 `<时长>s` 后面加 ` var(--ease-standard)`。

- [ ] **Step 4: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: style.css 新增圆角/间距/阴影分级/缓动曲线 token

--app-radius 及派生的 --radius-sm/md/lg；4px 基准间距 --space-1~8；
--elevation-1/2/3 三级阴影（浅深各一套）；--ease-standard/--ease-spring
两条缓动曲线。D3 节点矩形接入 --radius-sm，已有 transition 接入
--ease-standard。
EOF
)"
```

---

## Task 7: `style.css` 共享按钮与分段控件

**Files:**
- Modify: `frontend/src/style.css`

**Interfaces:**
- Consumes: Task 6 的 `--radius-sm`/`--space-*`/`--ease-standard`、Task 5 的颜色 token
- Produces: `.btn`/`.btn-primary`/`.btn-secondary`/`.btn-ghost`/`.btn-danger-ghost`（全局按钮类）、`.segmented`/`.segmented button`/`.segmented button.active`（全局分段控件类）——Task 11/12/13/14 里替换掉的散落按钮/分段实现都改用这些类名。

- [ ] **Step 1: 在 style.css 末尾新增"共享控件样式"小节**

在文件末尾（`.edge-handle:hover .edge-handle-dot` 规则之后）加：

```css
/* ── 共享控件样式（按钮 / 分段控件） ──────────────────────────────────────── */
/* 这些是全局类，不是某个组件的 scoped 样式——之前 priority-btn/recur-kind-btn/
   recur-weekday-btn/设置里的高亮模式各自实现了一遍几乎相同的"分段选中"样式，
   选中态的透明度还互相不统一（0.1/0.2/0.08），这里统一成一份。 */

.btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.9231rem;
    font-weight: 600;
    border: 1px solid transparent;
    cursor: pointer;
    transition:
        background 0.15s var(--ease-standard),
        border-color 0.15s var(--ease-standard),
        color 0.15s var(--ease-standard),
        transform 0.1s var(--ease-standard);
}
.btn:active {
    transform: scale(0.97);
}
.btn:disabled {
    opacity: 0.4;
    cursor: default;
    transform: none;
}

.btn-primary {
    background: var(--blue);
    color: var(--bg-panel);
}
.btn-primary:hover:not(:disabled) {
    opacity: 0.88;
}

.btn-secondary {
    background: transparent;
    border-color: var(--border);
    color: var(--fg);
}
.btn-secondary:hover:not(:disabled) {
    border-color: var(--fg-dark);
    background: var(--bg-dark);
}

.btn-ghost {
    background: var(--bg-dark);
    color: var(--fg-dim);
}
.btn-ghost:hover:not(:disabled) {
    background: var(--bg-select);
    color: var(--fg);
}

.btn-danger-ghost {
    background: transparent;
    border-color: var(--border);
    color: var(--red);
}
.btn-danger-ghost:hover:not(:disabled) {
    background: rgba(209, 36, 47, 0.1);
    border-color: var(--red);
}

/* 分段控件：外层灰底托盘 + 内部按钮，选中项浮起（中性选择）或按语义色着色（如优先级） */
.segmented {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    background: var(--bg-dark);
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
}
.segmented button {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    color: var(--fg-dim);
    font-size: 0.8462rem;
    font-weight: 600;
    border: none;
    background: transparent;
    cursor: pointer;
    transition: all 0.15s var(--ease-standard);
}
.segmented button:hover {
    color: var(--fg);
}
.segmented button.active {
    background: var(--bg-panel);
    color: var(--fg);
    box-shadow: var(--elevation-1);
}
```

- [ ] **Step 2: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功（这一步只是新增未被引用的全局类，不会报错）

- [ ] **Step 3: Commit**

```bash
git add frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: style.css 新增共享按钮和分段控件全局样式

.btn 系列（primary/secondary/ghost/danger-ghost）和 .segmented 分段
控件，替代此前在多个组件里各自重复实现的按钮/分段选中样式。这一步
只新增类，尚未替换任何组件里的旧实现（见后续任务）。
EOF
)"
```

---

## Task 8: `style.css` 外壳 token（界面风格：扁平 / 玻璃 / 新拟态）

**Files:**
- Modify: `frontend/src/style.css`

**Interfaces:**
- Consumes: Task 5/6 的颜色和圆角/阴影 token
- Produces: `--shell-bg`/`--shell-backdrop`/`--shell-border`/`--shell-shadow`/`--shell-shadow-pressed`/`--shell-radius`/`--shell-fg`/`--shell-fg-dim`（默认=扁平，`[data-ui-style="glass"]`/`[data-ui-style="neumorphism"]` 两组覆盖，含深色交叉）——Task 14/15/16 的外壳组件（顶栏/侧栏/统计卡）消费这些 token。

- [ ] **Step 1: 在 style.css 末尾（共享控件样式之后）加外壳 token**

```css
/* ── 外壳 token（界面风格：扁平 / 液态玻璃 / 新拟态） ──────────────────────── */
/* 只影响外壳（顶栏/侧栏/弹窗/下拉菜单/统计卡片），不影响 DAG 图任务节点的
   语义色——节点状态色（node-done 等）不读这套 token，永远走扁平语言。
   切换风格只是给 <html> 换一个 data-ui-style 属性，组件样式只认 --shell-*，
   不需要为每个风格单独写一份组件 CSS。 */

:root {
    --shell-bg: var(--bg-panel);
    --shell-backdrop: none;
    --shell-border: 1px solid var(--border);
    --shell-shadow: var(--elevation-1);
    --shell-shadow-pressed: var(--bg-select);
    --shell-radius: var(--radius-lg);
    --shell-fg: var(--fg);
    --shell-fg-dim: var(--fg-dim);
}

html[data-ui-style="glass"] {
    --shell-bg: rgba(255, 255, 255, 0.5);
    --shell-tint: rgba(255, 255, 255, 0.06);
    --shell-backdrop: blur(14px) saturate(1.3);
    --shell-border: 1px solid rgba(255, 255, 255, 0.14);
    --shell-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.45), 0 8px 24px rgba(16, 24, 40, 0.16);
    --shell-shadow-pressed: rgba(255, 255, 255, 0.16);
    --shell-radius: var(--radius-lg);
}
html[data-ui-style="glass"][data-theme-mode="dark"] {
    --shell-bg: rgba(20, 20, 26, 0.5);
    --shell-tint: rgba(255, 255, 255, 0.04);
    --shell-border: 1px solid rgba(255, 255, 255, 0.08);
    --shell-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08), 0 12px 32px rgba(0, 0, 0, 0.5);
    --shell-shadow-pressed: rgba(255, 255, 255, 0.1);
}

html[data-ui-style="neumorphism"] {
    --shell-bg: #e8ecf3;
    --shell-backdrop: none;
    --shell-border: none;
    --neu-shadow-d: rgba(163, 177, 198, 0.55);
    --neu-shadow-l: rgba(255, 255, 255, 0.9);
    --shell-shadow: 6px 6px 12px var(--neu-shadow-d), -6px -6px 12px var(--neu-shadow-l);
    --shell-shadow-pressed: inset 4px 4px 8px var(--neu-shadow-d), inset -4px -4px 8px var(--neu-shadow-l);
    --shell-radius: 16px;
    --shell-fg: #414a5e;
    --shell-fg-dim: #7c8598;
}
html[data-ui-style="neumorphism"][data-theme-mode="dark"] {
    --shell-bg: #2b2f3a;
    --neu-shadow-d: rgba(0, 0, 0, 0.6);
    --neu-shadow-l: rgba(255, 255, 255, 0.055);
    --shell-fg: #d7dbe6;
    --shell-fg-dim: #8a90a3;
}

/* 玻璃质感的三层结构（外侧投影+内侧高光+色调叠加），给外壳组件一个共享类，
   避免每个外壳组件都重复写一遍 ::before/::after。只在 data-ui-style="glass"
   时生效；扁平/新拟态下 ::before/::after 不产生内容，不影响布局。 */
.shell-surface {
    position: relative;
    isolation: isolate;
}
html[data-ui-style="glass"] .shell-surface::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 1;
    border-radius: inherit;
    background: var(--shell-tint);
    pointer-events: none;
}
html[data-ui-style="glass"] .shell-surface::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    backdrop-filter: var(--shell-backdrop);
    -webkit-backdrop-filter: var(--shell-backdrop);
}
```

- [ ] **Step 2: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 3: Commit**

```bash
git add frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: style.css 新增外壳 token，支撑扁平/玻璃/新拟态三种界面风格

--shell-bg/--shell-shadow/--shell-radius 等 token，默认(扁平)直接
复用已有的 bg-panel/elevation-1/radius-lg；[data-ui-style="glass"]
和 [data-ui-style="neumorphism"] 两组覆盖值，含深色交叉选择器。新增
.shell-surface 共享类封装玻璃的三层结构。只影响外壳组件，DAG 图节点
不接入这套 token。
EOF
)"
```

---

## Task 9: `useApi.js` 新增配色方案接口封装

**Files:**
- Modify: `frontend/src/composables/useApi.js`

**Interfaces:**
- Consumes: Task 4 的 Tauri 命令 `list_color_schemes`/`get_color_scheme`
- Produces: `listColorSchemes(): Promise<{id, name}[]>`、`getColorScheme(id: string): Promise<{name?, light?, dark?}>`——Task 10 的 `App.vue` 和 Task 11 的 `SettingsModal.vue` 直接 import 使用。

- [ ] **Step 1: 加两个封装函数**

在 `frontend/src/composables/useApi.js` 里 `listSystemFonts` 函数之后加：

```js
/** 列出所有可选配色方案：内置 3 套 + 数据目录 themes/ 下发现的自定义文件，每项 { id, name }。 */
export async function listColorSchemes() {
  return call("list_color_schemes");
}

/** 按 id 加载一个配色方案的完整内容：{ name?, light?, dark? }，id 形如 "builtin:b"/"custom:my-theme"。 */
export async function getColorScheme(id) {
  return call("get_color_scheme", { id });
}
```

- [ ] **Step 2: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 3: Commit**

```bash
git add frontend/src/composables/useApi.js && git commit -m "$(cat <<'EOF'
feat: useApi.js 新增 listColorSchemes/getColorScheme 封装
EOF
)"
```

---

## Task 10: `App.vue` 应用逻辑（配色方案 / 深浅模式 / 圆角 / 界面风格）

**Files:**
- Modify: `frontend/src/App.vue`

**Interfaces:**
- Consumes: Task 9 的 `listColorSchemes`/`getColorScheme`，Task 1 的 `Settings` 新字段
- Produces: `computeEffectiveMode(themeMode)`、`applyThemeMode(themeMode) -> effectiveMode`、`applyColorScheme(colorScheme, effectiveMode)`、`applyCornerRadius(px)`、`applyUiStyle(uiStyle)`——Task 11 的 `SettingsModal.vue` 保存后由 `App.vue` 调用这些函数，函数名和参数顺序不能改。

- [ ] **Step 1: 扩充 `settings` ref 的初始值**

在 `frontend/src/App.vue` 里，找到 `settings` ref 定义（`node_label_recur: constants.DEFAULT_NODE_LABELS.recur,` 那一行）之后加：

```js
    color_scheme: "",
    theme_mode: "light",
    corner_radius: 10,
    ui_style: "flat",
```

- [ ] **Step 2: 加应用逻辑函数**

`applyColorScheme` 加载失败时要给用户可见的提示，不能只是 `console.error`——`App.vue` 里已经有一个现成的悬浮错误通知机制：`const error = ref("");`（本文件已有定义，不用新建），配合一个 `watch(error, ...)` 在 `settings.value.notification_duration_seconds` 秒后自动清空。想触发一次错误提示，直接 `error.value = "<提示文案>"` 即可，不需要手动管理定时器。

在 `applyNodeFontFamily` 函数之后加：

```js
const COLOR_TOKEN_KEYS = [
    "bg", "bg-dark", "bg-panel", "bg-select", "bg-popup",
    "fg", "fg-dim", "fg-dark",
    "blue", "magenta", "cyan", "green", "yellow", "orange", "red",
    "border",
    "node-done", "node-today", "node-overdue", "node-locked", "node-waiting",
];

/** 根据 theme_mode 算出当前实际该用浅色还是深色 */
function computeEffectiveMode(themeMode) {
    if (themeMode === "light" || themeMode === "dark") return themeMode;
    return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
}

/** 把 data-theme-mode 属性写到 <html> 上，style.css 里两套默认值靠这个属性切换；返回算出的 effectiveMode 供 applyColorScheme 使用 */
function applyThemeMode(themeMode) {
    const effectiveMode = computeEffectiveMode(themeMode);
    document.documentElement.setAttribute("data-theme-mode", effectiveMode);
    return effectiveMode;
}

/**
 * 应用配色方案：先清空全部 21 个 token 的内联覆盖（否则从"方案 B"切到"方案 D"，
 * B 设置过、D 没设置的 key 会保留 B 的值，不会正确回落到 style.css 默认值），
 * 再按需要设置有值的 key。colorScheme 为空字符串表示用默认方案 A，什么都不用做。
 */
async function applyColorScheme(colorScheme, effectiveMode) {
    for (const key of COLOR_TOKEN_KEYS) {
        document.documentElement.style.removeProperty(`--${key}`);
    }
    if (!colorScheme) return;
    let scheme;
    try {
        scheme = await getColorScheme(colorScheme);
    } catch (e) {
        console.error("加载配色方案失败，回落到默认配色", e);
        error.value = "配色方案加载失败，已回落到默认配色";
        return;
    }
    const tokens = scheme[effectiveMode] || {};
    for (const [key, value] of Object.entries(tokens)) {
        if (value == null) continue;
        document.documentElement.style.setProperty(`--${key}`, value);
    }
}

/** 应用圆角设置：--app-radius 一个变量，--radius-sm/md/lg 在 CSS 里用 calc() 派生 */
function applyCornerRadius(px) {
    document.documentElement.style.setProperty("--app-radius", `${px}px`);
}

/** 应用界面风格：data-ui-style 属性驱动 style.css 里的 --shell-* token 切换 */
function applyUiStyle(uiStyle) {
    document.documentElement.setAttribute("data-ui-style", uiStyle);
}
```

在文件顶部的 `import { ... } from "./composables/useApi"`（或等价的具名导入列表）里加上 `getColorScheme`（`listColorSchemes` 在 Task 11 里由 `SettingsModal.vue` 自己引入，`App.vue` 只需要 `getColorScheme`）。

- [ ] **Step 3: 在 `onMounted` 里调用**

找到 `onMounted` 里加载设置之后的代码（`applyNodeFontFamily(settings.value.node_font_family, settings.value.font_family);` 那一行），之后加：

```js
        const effectiveMode = applyThemeMode(settings.value.theme_mode);
        await applyColorScheme(settings.value.color_scheme, effectiveMode);
        applyCornerRadius(settings.value.corner_radius);
        applyUiStyle(settings.value.ui_style);
        window
            .matchMedia("(prefers-color-scheme: dark)")
            .addEventListener("change", async () => {
                if (settings.value.theme_mode !== "system") return;
                const mode = applyThemeMode(settings.value.theme_mode);
                await applyColorScheme(settings.value.color_scheme, mode);
            });
```

（如果 `onMounted` 所在函数不是 `async`，把它改成 `async function` / `async () => {}`，`await getSettings()` 那一行本来就已经是 `await`，说明这个函数已经是异步的，不需要额外改动。）

- [ ] **Step 4: 在保存设置的回调里同样调用**

找到 `saveSettings` 保存成功后重新应用字体设置的那段代码（`applyNodeFontFamily(settings.value.node_font_family, settings.value.font_family);` 出现的第二处，在保存回调里），之后加同样的四行：

```js
        const effectiveMode = applyThemeMode(settings.value.theme_mode);
        await applyColorScheme(settings.value.color_scheme, effectiveMode);
        applyCornerRadius(settings.value.corner_radius);
        applyUiStyle(settings.value.ui_style);
```

（这个回调函数也要确认是 `async`，原因同 Step 3。）

- [ ] **Step 5: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 6: 用 `run` 技能实际跑起来验证**

启动应用（`cd src-tauri && cargo tauri dev`），确认：
- 应用能正常启动，默认就是方案 A 的浅色配色，没有报错弹窗
- 打开系统深色模式再重启应用（此时 `theme_mode` 还是默认 `"light"`），界面应该仍然是浅色——不受系统主题影响，因为默认值特意选了 `"light"` 不是 `"system"`

Expected: 应用正常启动且外观符合预期

- [ ] **Step 7: Commit**

```bash
git add frontend/src/App.vue && git commit -m "$(cat <<'EOF'
feat: App.vue 接入配色方案/深浅模式/圆角/界面风格的应用逻辑

新增 applyThemeMode/applyColorScheme/applyCornerRadius/applyUiStyle，
在 onMounted 和保存设置后调用；深浅模式用 data-theme-mode 属性切换
CSS 层叠中的默认值，配色方案只做内联覆盖的增量应用（先清空 21 个
token 再按需设置），两者互不干扰。跟随系统时监听
prefers-color-scheme 变化实时切换。
EOF
)"
```

---

## Task 11: `SettingsModal.vue` 外观设置区

**Files:**
- Modify: `frontend/src/components/SettingsModal.vue`

**Interfaces:**
- Consumes: Task 9 的 `listColorSchemes`，Task 7 的 `.segmented` 共享类
- Produces: `submit()` emit 的对象里新增 `color_scheme`/`theme_mode`/`corner_radius`/`ui_style` 四个字段——`App.vue` 监听 `save` 事件后拿到的 payload 里会带这四个字段（跟 `Settings` 结构体字段名一致）。

- [ ] **Step 1: 加本地 state 和配色方案列表**

在 `frontend/src/components/SettingsModal.vue` 的 `<script setup>` 里，`import { listSystemFonts } from "../composables/useApi";` 那一行改成：

```js
import { listSystemFonts, listColorSchemes } from "../composables/useApi";
```

在 `SECTIONS` 数组里加一项（放在"通用"之后）：

```js
const SECTIONS = [
    { key: "general", label: "通用" },
    { key: "appearance", label: "外观" },
    { key: "duration", label: "时长格式" },
    { key: "graph", label: "图谱显示" },
    { key: "about", label: "关于" },
];
```

在 `trashRetentionDays` 等 ref 定义附近加：

```js
// ----------------------------------------
// 外观：配色方案 / 深浅模式 / 圆角 / 界面风格
// ----------------------------------------
const colorScheme = ref("");
const themeMode = ref("light");
const cornerRadius = ref(10);
const uiStyle = ref("flat");

const colorSchemeOptions = ref([{ id: "", name: "默认（克制中性）" }]);
async function loadColorSchemesOnce() {
    try {
        const list = await listColorSchemes();
        colorSchemeOptions.value = [
            { id: "", name: "默认（克制中性）" },
            ...list,
        ];
    } catch {
        colorSchemeOptions.value = [{ id: "", name: "默认（克制中性）" }];
    }
}

const themeModeOptions = [
    { key: "system", label: "跟随系统" },
    { key: "light", label: "浅色" },
    { key: "dark", label: "深色" },
];

const uiStyleOptions = [
    { key: "flat", label: "扁平" },
    { key: "glass", label: "液态玻璃" },
    { key: "neumorphism", label: "新拟态" },
];
```

- [ ] **Step 2: 在 `watch(props.visible)` 里回填并每次打开都刷新配色列表**

在 `watch` 回调的最后（`nodeLabelRecur.value = ...` 之后）加：

```js
        colorScheme.value = props.settings.color_scheme || "";
        themeMode.value = props.settings.theme_mode || "light";
        cornerRadius.value = props.settings.corner_radius ?? 10;
        uiStyle.value = props.settings.ui_style || "flat";
        loadColorSchemesOnce();
```

- [ ] **Step 3: 在 `submit()` 的 emit 对象里加四个字段**

在 `emit("save", { ... })` 对象里，`node_label_recur: ...` 之后加：

```js
        color_scheme: colorScheme.value,
        theme_mode: themeMode.value,
        corner_radius: Math.min(24, Math.max(0, Math.round(Number(cornerRadius.value) || 10))),
        ui_style: uiStyle.value,
```

- [ ] **Step 4: 加模板里的"外观"分区**

在模板里 `<template v-if="activeSection === 'general'">...</template>` 整块之后（`</template>`，`<template v-else-if="activeSection === 'duration'">` 之前）加一个新分区：

```html
                        <template v-else-if="activeSection === 'appearance'">
                            <div class="form-row">
                                <label class="form-label">
                                    配色方案
                                    <span class="form-hint">
                                        内置预设，或数据目录 themes/ 下你自己放的自定义文件
                                    </span>
                                </label>
                                <select v-model="colorScheme" class="form-input">
                                    <option
                                        v-for="opt in colorSchemeOptions"
                                        :key="opt.id"
                                        :value="opt.id"
                                    >
                                        {{ opt.name }}
                                    </option>
                                </select>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    深浅模式
                                </label>
                                <div class="segmented">
                                    <button
                                        v-for="m in themeModeOptions"
                                        :key="m.key"
                                        type="button"
                                        :class="{ active: themeMode === m.key }"
                                        @click="themeMode = m.key"
                                    >
                                        {{ m.label }}
                                    </button>
                                </div>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    圆角
                                    <span class="form-hint">
                                        影响按钮/输入框/卡片/图节点的圆角，0-24
                                    </span>
                                </label>
                                <input
                                    v-model.number="cornerRadius"
                                    type="range"
                                    min="0"
                                    max="24"
                                />
                                <span class="form-hint">{{ cornerRadius }}px</span>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    界面风格
                                    <span class="form-hint">
                                        只影响顶栏/侧栏/弹窗/下拉菜单/统计卡片，不影响图谱任务节点的状态色
                                    </span>
                                </label>
                                <div class="segmented">
                                    <button
                                        v-for="s in uiStyleOptions"
                                        :key="s.key"
                                        type="button"
                                        :class="{ active: uiStyle === s.key }"
                                        @click="uiStyle = s.key"
                                    >
                                        {{ s.label }}
                                    </button>
                                </div>
                            </div>
                        </template>
```

- [ ] **Step 5: 把已有的"高亮模式"分段控件迁移到共享 `.segmented` 类**

这一步顺手把 Task 7 新增的共享分段控件用在已有的"高亮模式"上，减少一份重复实现。把 `general` 分区里原来的：

```html
                                <div class="mode-group">
                                    <button
                                        v-for="m in highlightModeOptions"
                                        :key="m.key"
                                        class="mode-btn"
                                        :class="{
                                            active: highlightMode === m.key,
                                        }"
                                        @click="
                                            emit(
                                                'update:highlight-mode',
                                                m.key,
                                            )
                                        "
                                    >
                                        {{ m.label }}
                                    </button>
                                </div>
```

改成：

```html
                                <div class="segmented">
                                    <button
                                        v-for="m in highlightModeOptions"
                                        :key="m.key"
                                        type="button"
                                        :class="{
                                            active: highlightMode === m.key,
                                        }"
                                        @click="
                                            emit(
                                                'update:highlight-mode',
                                                m.key,
                                            )
                                        "
                                    >
                                        {{ m.label }}
                                    </button>
                                </div>
```

`<style scoped>` 里原来给 `.mode-group`/`.mode-btn` 写的规则可以删掉（改用全局 `.segmented`），如果 `.mode-group`/`.mode-btn` 在文件里没有被其它地方引用的话；用 `grep -n "mode-group\|mode-btn" frontend/src/components/SettingsModal.vue` 确认没有其它引用后再删。

- [ ] **Step 6: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 7: 用 `run` 技能实际验证**

启动应用，打开设置弹窗：
- 左侧菜单出现"外观"分区
- 配色方案下拉框能看到"默认（克制中性）"和三个内置预设
- 切换深浅模式、拖动圆角滑块、切换界面风格，点保存后界面立即变化
- "高亮模式"分段控件视觉上变成新的浮起选中样式，功能不变（选中项仍能正确驱动图谱链路高亮范围）

Expected: 以上全部符合

- [ ] **Step 8: Commit**

```bash
git add frontend/src/components/SettingsModal.vue && git commit -m "$(cat <<'EOF'
feat: SettingsModal 新增外观设置区，高亮模式迁移到共享分段控件

新增配色方案下拉、深浅模式/界面风格分段控件、圆角滑块；"高亮模式"
从专属的 mode-group/mode-btn 迁移到共享的 .segmented 类。
EOF
)"
```

---

## Task 12: `TaskFormModal.vue` 按钮修复与分段控件迁移

**Files:**
- Modify: `frontend/src/components/TaskFormModal.vue`

**Interfaces:**
- Consumes: Task 7 的 `.btn`/`.btn-primary`/`.btn-secondary`/`.segmented`

- [ ] **Step 1: 修复取消/确认按钮混用同一个 class 的 bug**

把模板里的：

```html
                <div class="modal-footer">
                    <button class="btn-submit" @click="emit('close')">
                        取消
                    </button>
                    <button
                        class="btn-submit"
                        @click="submit"
                        :disabled="!description.trim()"
                    >
                        {{ isModify ? "保存修改" : "添加任务" }}
                    </button>
                </div>
```

改成：

```html
                <div class="modal-footer">
                    <button class="btn btn-secondary" @click="emit('close')">
                        取消
                    </button>
                    <button
                        class="btn btn-primary"
                        @click="submit"
                        :disabled="!description.trim()"
                    >
                        {{ isModify ? "保存修改" : "添加任务" }}
                    </button>
                </div>
```

- [ ] **Step 2: 删除 `<style scoped>` 里不再需要的 `.btn-submit`/`.btn-cancel` 规则**

先用 `grep -n "btn-submit\|btn-cancel" frontend/src/components/TaskFormModal.vue` 确认这两个 class 在模板里没有其它使用（`.btn-cancel` 本来就是没被用上的死代码），确认后删掉 `<style scoped>` 里这两条规则块。

- [ ] **Step 3: 优先级分段控件迁移到共享 `.segmented` 类**

把模板里的：

```html
                                <div class="priority-group">
```

改成：

```html
                                <div class="segmented">
```

（`priority-group` 内部的 `v-for` 循环、`priority-btn` 相关的 class 绑定逻辑本身不变——只是外层容器类名换掉；`priority-btn` 上原有的 `active priority-h/m/l/none` 组合 class 继续保留，用来叠加语义色，跟共享 `.segmented button.active` 的中性浮起样式不冲突，因为语义色版本的选择器优先级更高。）

在 `<style scoped>` 里，删掉原来的：

```css
/* 优先级单选组 */
.priority-group {
    display: flex;
    gap: 6px;
}
.priority-btn {
    padding: 5px 16px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.9231rem;
    font-weight: 600;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.priority-btn:hover {
    border-color: var(--fg-dark);
    color: var(--fg);
}
```

改成（只保留语义色叠加，尺寸/圆角/hover 交给共享 `.segmented button`）：

```css
/* 优先级：容器用共享 .segmented，这里只叠加语义色（中性选中态之外的着色） */
.priority-btn.active.priority-h {
    background: rgba(209, 36, 47, 0.14);
    color: var(--red);
    box-shadow: none;
}
.priority-btn.active.priority-m {
    background: rgba(154, 103, 0, 0.14);
    color: var(--yellow);
    box-shadow: none;
}
.priority-btn.active.priority-l {
    background: rgba(79, 107, 255, 0.14);
    color: var(--blue);
    box-shadow: none;
}
.priority-btn.active.priority-none {
    background: var(--bg-select);
    color: var(--fg);
    box-shadow: none;
}
```

（这四个语义色改用 `var(--red)`/`var(--yellow)`/`var(--blue)` 派生的 `rgba`，而不是之前硬编码的 `rgba(247,118,142,.2)` 这类跟 `--red` 完全对不上的值——这样切配色方案时这几个浅色底也会跟着变。）

- [ ] **Step 4: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 5: 用 `run` 技能验证**

打开"添加任务"弹窗：
- "取消"按钮是描边样式（次要），"添加任务"是实心蓝色（主要），两者视觉上能区分主次
- 优先级选择四个选项样式正常，点选"高"能看到浅红色底、"中"浅黄色底、"低"浅蓝色底

Expected: 以上全部符合

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/TaskFormModal.vue && git commit -m "$(cat <<'EOF'
fix: TaskFormModal 修复取消/确认按钮误用同一 class 的问题

取消按钮此前和确认按钮共用 btn-submit，视觉上分不清主次操作；改用
共享的 .btn-secondary/.btn-primary。优先级选择器迁移到共享
.segmented 容器，语义色底色改用 --red/--yellow/--blue 派生，不再
硬编码跟配色变量对不上的 rgba 值。
EOF
)"
```

---

## Task 13: 其余弹窗迁移到共享按钮样式

**Files:**
- Modify: `frontend/src/components/ConfirmDialog.vue`
- Modify: `frontend/src/components/TagManagerModal.vue`
- Modify: `frontend/src/components/TimeEntryNoteModal.vue`

**Interfaces:**
- Consumes: Task 7 的 `.btn`/`.btn-primary`/`.btn-secondary`/`.btn-danger-ghost`

- [ ] **Step 1: `ConfirmDialog.vue`**

先读一遍确认按钮结构：`grep -n "background: var(--blue)\|background: var(--red)\|<button" frontend/src/components/ConfirmDialog.vue`。把模板里"取消"按钮的 class 换成 `btn btn-secondary`，"确认"按钮换成 `btn btn-primary`（如果是普通确认）或 `btn btn-danger-ghost`（如果 `<style scoped>` 里原本 `background: var(--red)`，说明这是一个危险操作确认，用 `btn` + 内联 `style="background:var(--red);color:var(--bg-panel)"` 的方式保留红色强调，同时删掉 `<style scoped>` 里原来给这两个按钮写的 `padding`/`border-radius` 规则（改由共享 `.btn` 提供）。

- [ ] **Step 2: `TagManagerModal.vue` 和 `TimeEntryNoteModal.vue`**

用同样的方法处理：`grep -n "<button" frontend/src/components/TagManagerModal.vue frontend/src/components/TimeEntryNoteModal.vue` 找到各自的确认/取消/删除按钮，按下面的映射换 class：
- 主要操作（保存/确认）→ `btn btn-primary`
- 次要操作（取消/关闭）→ `btn btn-secondary`
- 危险操作（删除标签）→ `btn btn-danger-ghost`

每个文件里，`<style scoped>` 里原本单独给这些按钮写的 `padding`/`border-radius`/`background`/`transition` 规则要删掉（换成共享 `.btn` 提供的），只保留该文件独有的定位/间距类规则（比如按钮所在容器的 `display: flex; gap: ...`）。

- [ ] **Step 2: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 3: 用 `run` 技能验证**

依次触发这三个弹窗（删除任务确认、标签管理、结束计时后填写总结），确认按钮视觉统一（主要操作实心蓝、次要操作描边、危险操作红色），功能不变。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/ConfirmDialog.vue frontend/src/components/TagManagerModal.vue frontend/src/components/TimeEntryNoteModal.vue && git commit -m "$(cat <<'EOF'
refactor: ConfirmDialog/TagManagerModal/TimeEntryNoteModal 按钮迁移到共享样式

删掉各自重复的按钮 padding/border-radius/transition 规则，统一用
.btn-primary/.btn-secondary/.btn-danger-ghost。
EOF
)"
```

---

## Task 14: 顶栏（`App.vue`）外壳 token 与图标统一

**Files:**
- Modify: `frontend/src/App.vue`
- Modify: `frontend/src/style.css`

**Interfaces:**
- Consumes: Task 8 的 `--shell-*` token
- Produces: `frontend/src/style.css` 里的图标 `<symbol>` 集合（`#i-refresh`/`#i-tag`/`#i-settings`），Task 15/16 如果需要同类图标可以复用

- [ ] **Step 1: 在 `App.vue` 模板顶部加一次性的图标 sprite**

在 `App.vue` 模板的根元素刚开始的位置（比如顶栏 `<div class="topbar">` 之前）加一个隐藏的 SVG sprite：

```html
        <svg style="position: absolute; width: 0; height: 0" aria-hidden="true">
            <defs>
                <g id="i-refresh" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M3 11a7 7 0 0 1 12.1-4.9M17 3v4h-4" />
                    <path d="M17 9a7 7 0 0 1-12.1 4.9M3 17v-4h4" />
                </g>
                <g id="i-tag" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round">
                    <path d="M10.5 3H16a1 1 0 0 1 1 1v5.5a1 1 0 0 1-.3.7l-7 7a1 1 0 0 1-1.4 0l-5.5-5.5a1 1 0 0 1 0-1.4l7-7a1 1 0 0 1 .7-.3z" />
                    <circle cx="13.2" cy="6.8" r="1" fill="currentColor" stroke="none" />
                </g>
                <g id="i-settings" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
                    <line x1="3" y1="6" x2="8" y2="6" /><circle cx="10.5" cy="6" r="2" /><line x1="13" y1="6" x2="17" y2="6" />
                    <line x1="3" y1="14" x2="4" y2="14" /><circle cx="6.5" cy="14" r="2" /><line x1="9" y1="14" x2="17" y2="14" />
                </g>
            </defs>
        </svg>
```

- [ ] **Step 2: 替换刷新/标签/设置按钮的图标**

把：

```html
            <button class="btn-refresh" @click="load" :disabled="loading">
                {{ loading ? "加载中…" : "↺ 刷新" }}
            </button>
```

改成：

```html
            <button class="btn btn-ghost" @click="load" :disabled="loading">
                <svg class="icon" viewBox="0 0 20 20"><use href="#i-refresh" /></svg>
                {{ loading ? "加载中…" : "刷新" }}
            </button>
```

把：

```html
            <button
                class="btn-refresh"
                title="标签管理"
                @click="showTagManager = true"
            >
                🏷 标签
            </button>
```

改成：

```html
            <button
                class="btn btn-ghost"
                title="标签管理"
                @click="showTagManager = true"
            >
                <svg class="icon" viewBox="0 0 20 20"><use href="#i-tag" /></svg>
                标签
            </button>
```

把：

```html
            <button
                class="btn-refresh"
                title="设置"
                @click="showSettings = true"
            >
                ⚙ 设置
            </button>
```

改成：

```html
            <button
                class="btn btn-ghost"
                title="设置"
                @click="showSettings = true"
            >
                <svg class="icon" viewBox="0 0 20 20"><use href="#i-settings" /></svg>
                设置
            </button>
```

把"添加任务"按钮：

```html
            <button class="btn-add-toggle" @click="openAdd">+ 添加任务</button>
```

改成：

```html
            <button class="btn btn-primary" @click="openAdd">+ 添加任务</button>
```

在 `<style scoped>` 里删掉不再使用的 `.btn-refresh`/`.btn-add-toggle` 规则（先用 `grep -n "btn-refresh\|btn-add-toggle" frontend/src/App.vue` 确认没有其它引用）。

- [ ] **Step 3: 在 `style.css` 加 `.icon` 通用类**

在共享控件样式小节（Task 7 加的那部分）里加：

```css
.icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    vertical-align: -2px;
}
```

- [ ] **Step 4: 顶栏容器接入外壳 token**

在 `App.vue` 的 `<style scoped>` 里找到顶栏容器规则（class 类似 `.topbar` 或 `.header`，用 `grep -n "class=\"topbar\"" frontend/src/App.vue` 确认实际用的 class 名），给它加上（如果原来是硬编码 `background`/`box-shadow`/`border-radius`，改成引用外壳 token；如果原来没有这些属性，新增）：

```css
.topbar {
    background: var(--shell-bg);
    backdrop-filter: var(--shell-backdrop);
    -webkit-backdrop-filter: var(--shell-backdrop);
    box-shadow: var(--shell-shadow);
    color: var(--shell-fg);
}
```

并在模板里给顶栏容器加 `shell-surface` class（跟原有 class 并列，比如 `class="topbar shell-surface"`），让玻璃风格的三层结构（Task 8 的 `.shell-surface::before/::after`）生效。

- [ ] **Step 5: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 6: 用 `run` 技能验证**

启动应用：
- 顶栏"刷新/标签/设置"三个图标视觉风格一致（都是细线条），不再有 emoji 和符号混用
- 设置里把"界面风格"切到"液态玻璃"，顶栏出现半透明模糊效果；切到"新拟态"，顶栏出现柔和的浮起阴影；切回"扁平"，恢复纯色背景

Expected: 以上全部符合

- [ ] **Step 7: Commit**

```bash
git add frontend/src/App.vue frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: 顶栏图标统一为线性 SVG 图标，接入外壳 token

刷新/标签/设置三个按钮的 emoji/Unicode 符号混用问题（🏷 是全彩
emoji，↺/⚙ 是纯线条符号）改成同一套手绘 SVG 图标；顶栏容器接入
--shell-* token，界面风格设置能实际影响顶栏外观。
EOF
)"
```

---

## Task 15: 侧边栏项目树外壳 token

**Files:**
- Modify: `frontend/src/components/ProjectTree.vue`
- Modify: `frontend/src/components/ProjectTreeNode.vue`
- Modify: `frontend/src/components/ProjectContextMenu.vue`

**Interfaces:**
- Consumes: Task 8 的 `--shell-*` token

- [ ] **Step 1: 侧边栏容器接入外壳 token**

在 `ProjectTree.vue` 里找到侧边栏最外层容器的 `<style scoped>` 规则（用 `grep -n "^\.\w* {" frontend/src/components/ProjectTree.vue | head -5` 定位第一个顶层容器类），给它加：

```css
background: var(--shell-bg);
backdrop-filter: var(--shell-backdrop);
-webkit-backdrop-filter: var(--shell-backdrop);
color: var(--shell-fg);
```

并在模板里给这个容器加 `shell-surface` class。

- [ ] **Step 2: 右键菜单（`ProjectContextMenu.vue`）接入外壳 token**

同样地，找到菜单容器的规则，加上：

```css
background: var(--shell-bg);
backdrop-filter: var(--shell-backdrop);
-webkit-backdrop-filter: var(--shell-backdrop);
box-shadow: var(--shell-shadow);
color: var(--shell-fg);
```

模板里加 `shell-surface` class。

- [ ] **Step 3: 项目树节点（`ProjectTreeNode.vue`）圆角/间距 token 化**

用 `grep -n "border-radius\|padding\|margin" frontend/src/components/ProjectTreeNode.vue` 找出硬编码的圆角和间距值，把 `border-radius` 换成 `var(--radius-sm)`，把明显是"4/6/8/12/16px"倍数关系的 `padding`/`margin` 值换成对应的 `var(--space-N)`。

- [ ] **Step 4: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 5: 用 `run` 技能验证**

在"任务看板"页面，确认左侧项目树侧边栏、右键菜单在切换界面风格（扁平/玻璃/新拟态）时外观相应变化，项目树节点本身的间距/圆角看起来比之前更规整。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/ProjectTree.vue frontend/src/components/ProjectTreeNode.vue frontend/src/components/ProjectContextMenu.vue && git commit -m "$(cat <<'EOF'
feat: 侧边栏项目树和右键菜单接入外壳 token 与间距/圆角 token
EOF
)"
```

---

## Task 16: Dashboard 统计卡片外壳 token 与数字步进器组件

**Files:**
- Modify: `frontend/src/components/Dashboard.vue`
- Modify: `frontend/src/components/SettingsModal.vue`
- Modify: `frontend/src/style.css`

**Interfaces:**
- Consumes: Task 8 的 `--shell-*` token
- Produces: `.stepper`/`.stepper button`/`.stepper input`（全局共享类，供任何数字输入场景复用）

- [ ] **Step 1: 统计卡片接入外壳 token**

在 `Dashboard.vue` 里找到统计卡片的 `<style scoped>` 规则（用 `grep -n "border-radius\|border:" frontend/src/components/Dashboard.vue | head -10` 定位），把原来的 `background: var(--bg-panel); border: 1px solid var(--border);` 改成：

```css
background: var(--shell-bg);
backdrop-filter: var(--shell-backdrop);
-webkit-backdrop-filter: var(--shell-backdrop);
box-shadow: var(--shell-shadow);
border-radius: var(--shell-radius);
transition: box-shadow 0.2s var(--ease-standard), transform 0.2s var(--ease-standard);
color: var(--shell-fg);
```

给卡片加一个 hover 态（扁平/玻璃风格下悬浮时阴影加深、轻微上浮；新拟态下悬浮切换成凹陷提示"这是可交互的"）：

```css
.stat-card:hover {
    box-shadow: var(--elevation-2);
    transform: translateY(-1px);
}
html[data-ui-style="neumorphism"] .stat-card:hover {
    box-shadow: var(--shell-shadow-pressed);
    transform: none;
}
```

（`.stat-card` 换成实际的类名——用上面 grep 的结果确认。）模板里给每张卡片加 `shell-surface` class。

- [ ] **Step 2: 在 style.css 加共享的数字步进器样式**

在共享控件样式小节里加：

```css
.stepper {
    display: inline-flex;
    align-items: stretch;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
}
.stepper button {
    width: 28px;
    border: none;
    background: var(--bg-dark);
    color: var(--fg-dim);
    font-size: 14px;
    cursor: pointer;
    transition: background 0.15s var(--ease-standard), color 0.15s var(--ease-standard);
}
.stepper button:hover {
    background: var(--border);
    color: var(--fg);
}
.stepper input {
    width: 52px;
    border: none;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    text-align: center;
    background: var(--bg-panel);
    color: var(--fg);
    font: inherit;
    padding: 6px 0;
}
.stepper input:focus {
    outline: none;
}
```

- [ ] **Step 3: `SettingsModal.vue` 的"废纸篓保留天数"和"字体大小"改用步进器**

把 `frontend/src/components/SettingsModal.vue` 里原来的：

```html
                                <input
                                    v-model.number="trashRetentionDays"
                                    type="number"
                                    min="0"
                                    max="3650"
                                    class="form-input"
                                />
```

改成：

```html
                                <div class="stepper">
                                    <button
                                        type="button"
                                        @click="trashRetentionDays = Math.max(0, trashRetentionDays - 1)"
                                    >
                                        −
                                    </button>
                                    <input v-model.number="trashRetentionDays" />
                                    <button
                                        type="button"
                                        @click="trashRetentionDays = Math.min(3650, trashRetentionDays + 1)"
                                    >
                                        +
                                    </button>
                                </div>
```

同样地把"字体大小"的：

```html
                                <input
                                    v-model.number="fontSize"
                                    type="number"
                                    min="8"
                                    max="32"
                                    class="form-input"
                                />
```

改成：

```html
                                <div class="stepper">
                                    <button
                                        type="button"
                                        @click="fontSize = Math.max(8, fontSize - 1)"
                                    >
                                        −
                                    </button>
                                    <input v-model.number="fontSize" />
                                    <button
                                        type="button"
                                        @click="fontSize = Math.min(32, fontSize + 1)"
                                    >
                                        +
                                    </button>
                                </div>
```

- [ ] **Step 4: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 5: 用 `run` 技能验证**

首页统计卡片在切换界面风格时外观相应变化（玻璃=半透明模糊，新拟态=浮起阴影，鼠标悬浮有反馈）；设置弹窗里"废纸篓保留天数"和"字体大小"变成 −/输入框/+ 的步进器样式，点击 +/− 能正确加减，直接在输入框里打字修改也仍然生效。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/Dashboard.vue frontend/src/components/SettingsModal.vue frontend/src/style.css && git commit -m "$(cat <<'EOF'
feat: 统计卡片接入外壳 token，数字输入改用自定义步进器

统计卡片支持三种界面风格切换、加悬浮反馈；新增共享 .stepper 组件
替代原生 <input type=number> 的浏览器默认上下箭头（无法定制样式、
跟整体扁平风格不一致）。
EOF
)"
```

---

## Task 17: `TaskGraph.vue` 节点圆角与剩余组件的 token 扫尾

**Files:**
- Modify: `frontend/src/components/TaskGraph.vue`
- Modify: `frontend/src/components/TaskDetail.vue`
- Modify: `frontend/src/components/TaskListView.vue`
- Modify: `frontend/src/components/ChartsPage.vue`
- Modify: `frontend/src/components/CalendarPage.vue`
- Modify: `frontend/src/components/ColorSwatchPicker.vue`
- Modify: `frontend/src/components/IconPicker.vue`
- Modify: `frontend/src/components/DatePicker.vue`

**Interfaces:**
- Consumes: Task 6 的 `--radius-*`/`--space-*`/`--ease-standard`

- [ ] **Step 1: `TaskGraph.vue` 节点矩形接入圆角**

用 `grep -n "append(\"rect\")\|\.attr(\"rx\"" frontend/src/components/TaskGraph.vue` 找到 D3 创建节点 `<rect>` 的代码。任务节点矩形目前没有设置 `rx` 属性（圆角完全靠 Task 6 加的 CSS `rx: var(--radius-sm)` 生效，SVG 的 `rx` 几何属性可以被 CSS 覆盖，不需要在 JS 里额外设置）——**这一步只需要确认** Task 6 加的 `.node rect { rx: var(--radius-sm); }` 规则在浏览器里确实生效（用 `run` 技能打开应用，浏览器里没有开发者工具的话，靠视觉确认节点矩形四角是圆的），不需要改 `TaskGraph.vue` 本身的 JS 代码。如果发现没生效（比如某些 WebView 不支持 CSS 的 `rx` 几何属性），再回来在 D3 创建节点的代码里加 `.attr("rx", "var(--radius-sm)")` 作为兜底（SVG 属性也能直接写 CSS 变量表达式）。

- [ ] **Step 2: 剩余组件的圆角/间距/动效扫尾**

对 `TaskDetail.vue`、`TaskListView.vue`、`ChartsPage.vue`、`CalendarPage.vue`、`ColorSwatchPicker.vue`、`IconPicker.vue`、`DatePicker.vue` 这几个文件，逐个执行：

1. `grep -n "border-radius:\s*[0-9]" <file>`，把数值型的 `border-radius` 按视觉量级归类换成 `var(--radius-sm)`（≤6px）、`var(--radius-md)`（7-10px）或 `var(--radius-lg)`（>10px）。
2. `grep -n "box-shadow:" <file>`，如果是"弹出层/悬浮卡片"性质的阴影，换成 `var(--elevation-2)` 或 `var(--elevation-3)`（弹出层用 3）；如果是很轻的强调用阴影（比如 focus 态的细描边阴影），保留不动——不是所有 `box-shadow` 都要换成分级阴影，只换那些明显是"给这个面板一个立体感"的。
3. `grep -n "transition:" <file>`，给已有的 `transition` 声明补上 `var(--ease-standard)`（时长后面加一个空格再加这个变量），跟 Task 6 处理 `style.css` 里过渡效果的方式一致。

- [ ] **Step 3: `pnpm run build` 确认通过**

Run: `cd frontend && pnpm run build`
Expected: 构建成功

- [ ] **Step 4: 用 `run` 技能做一次全面视觉核对**

依次打开首页、任务看板（DAG 图）、分析、日历四个页面，以及任务详情面板、任务列表视图、颜色选择器、图标选择器、日期选择器，确认：
- 圆角/间距/过渡观感统一，没有明显不一致的"漏改"角落
- DAG 图任务节点四角是圆的，且圆角大小跟随设置里的圆角滑块变化
- 切换配色方案（B/C/D/自定义）时，DAG 图节点状态色（完成/超时/今日/锁定）会跟着变

Expected: 以上全部符合

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/TaskDetail.vue frontend/src/components/TaskListView.vue frontend/src/components/ChartsPage.vue frontend/src/components/CalendarPage.vue frontend/src/components/ColorSwatchPicker.vue frontend/src/components/IconPicker.vue frontend/src/components/DatePicker.vue && git commit -m "$(cat <<'EOF'
refactor: 剩余组件的圆角/阴影/动效改用统一 token

TaskDetail/TaskListView/ChartsPage/CalendarPage/ColorSwatchPicker/
IconPicker/DatePicker 里硬编码的 border-radius/box-shadow/transition
值换成 --radius-*/--elevation-*/--ease-standard。
EOF
)"
```

---

## Task 18: 变更日志与收尾验证

**Files:**
- Modify: `CHANGELOG.md`（项目根目录下如果不存在则新建）

- [ ] **Step 1: 检查 `CHANGELOG.md` 是否存在**

Run: `ls CHANGELOG.md 2>/dev/null || echo "不存在"`

如果不存在，创建 `CHANGELOG.md`：

```markdown
# Changelog

## 未发布
```

- [ ] **Step 2: 补一条完整的变更记录**

在"未发布"章节下加（贴合本文档 Task 1-17 的实际改动范围，具体到文件/函数/设置项级别；如果文件里已有其它未发布条目，追加在下面，不要覆盖）：

```markdown
### 新增
- 配色方案设置：内置默认（克制中性）+ 三套内置预设（沉稳深色系统/柔和明快/极简黑白灰）+ 自定义 TOML 文件（放进数据目录 `themes/` 下，未设置的颜色项自动回落默认值），新增 `Settings.color_scheme` 字段、`list_color_schemes`/`get_color_scheme` 命令、`src-tauri/src/color_scheme.rs` 模块
- 深浅模式设置（跟随系统/浅色/深色），新增 `Settings.theme_mode` 字段，`style.css` 新增 `:root[data-theme-mode="dark"]` 默认深色配色块
- 圆角设置（0-24px 滑块），新增 `Settings.corner_radius` 字段和 `--app-radius`/`--radius-sm/md/lg` CSS token
- 界面风格设置（扁平/液态玻璃/新拟态三选一，只影响顶栏/侧栏/弹窗/下拉菜单/统计卡片，不影响 DAG 图任务节点状态色），新增 `Settings.ui_style` 字段和 `--shell-*` CSS token
- 共享按钮样式（`.btn-primary/secondary/ghost/danger-ghost`）、共享分段控件样式（`.segmented`）、共享数字步进器（`.stepper`），替代此前多处重复实现的按钮/分段选中/数字输入样式
- 间距（`--space-1`~`--space-8`）、阴影分级（`--elevation-1/2/3`）、缓动曲线（`--ease-standard`/`--ease-spring`）CSS token

### 变更
- `style.css` 默认配色改为新的"克制中性"方案（原 GitHub 经典配色），`.rect-done`/`.rect-today`/`.rect-overdue`/`.rect-locked`/`.rect-waiting` 的硬编码颜色提取成 `--node-*` CSS 变量
- 顶栏刷新/标签/设置图标从 emoji/Unicode 符号混用统一为同一套线性 SVG 图标
- 设置弹窗新增"外观"分区；"高亮模式"从专属的 `mode-group`/`mode-btn` 迁移到共享 `.segmented`
- `TaskFormModal.vue` 优先级选择器迁移到共享 `.segmented`，语义色底色改用 `--red`/`--yellow`/`--blue` 派生而非硬编码 rgba
- `ConfirmDialog`/`TagManagerModal`/`TimeEntryNoteModal` 按钮样式迁移到共享 `.btn-*` 类

### 修复
- `TaskFormModal.vue` 里"取消"按钮误用 `btn-submit` class 导致和"添加任务"按钮渲染成一样的样式，用户分不清主次操作
```

- [ ] **Step 3: 最终全量验证**

Run: `cd src-tauri && cargo check && cargo test`
Expected: 编译通过，全部测试 PASS

Run: `cd frontend && pnpm run build`
Expected: 构建成功

用 `run` 技能完整启动一次应用，走一遍：默认配色 → 切换 B/C/D 配色 → 切换深浅模式 → 拖动圆角滑块 → 切换扁平/玻璃/新拟态三种界面风格 → 放一个自定义 TOML 主题文件进 `themes/` 目录验证能被发现和应用。

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md && git commit -m "$(cat <<'EOF'
docs: 补充视觉设计体系重做的变更日志
EOF
)"
```

---

## Self-Review Notes

- **Spec 覆盖**：设计文档第 3 节（配色方案）→ Task 1-4、9-11；第 4 节（圆角）→ Task 6、10、11；第 5 节（全局打磨）→ Task 6、7、17；第 6 节（界面风格）→ Task 8、10、11、14-16；第 7 节文件清单 → 对应分散在各任务的 Files 里；第 8 节验证方式 → 融进各任务的 Step 和 Task 18。
- **占位符检查**：每个 Step 都给了具体代码/命令，没有"添加适当的校验""类似 Task N"这类占位说法。
- **类型一致性**：`applyColorScheme`/`applyCornerRadius`/`applyUiStyle`/`computeEffectiveMode`/`applyThemeMode` 这五个函数名和参数在 Task 10（定义）与后续任务（不直接调用，但 `App.vue` 内部一致）里保持一致；`ColorTokens`/`ColorScheme`/`ColorSchemeInfo`/`is_valid_color`/`themes_dir`/`list_all`/`get` 在 Task 2/3/4 之间保持一致；`listColorSchemes`/`getColorScheme` 在 Task 9（定义）、Task 10/11（使用）之间保持一致；CSS token 名（`--shell-*`/`--radius-*`/`--space-*`/`--elevation-*`/`--ease-*`）在 Task 6/7/8（定义）与 Task 12-17（消费）之间保持一致。
