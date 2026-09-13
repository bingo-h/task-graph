# 视觉设计体系重做：配色方案 / 深浅模式 / 圆角 / 全局打磨

日期：2026-09-13
状态：待用户审阅

## 1. 背景与目标

当前前端只有一套写死在 `frontend/src/style.css` 的浅色配色（GitHub 经典配色路线），没有暗色模式，圆角/阴影/间距/动效散落在各个组件的 `<style scoped>` 里各写各的，没有统一体系。

这次要做三件事：

1. 让配色变得**可切换、可自定义**——用户能在内置的几套预设之间切换，也能自己写一个 TOML 文件放进数据目录来自定义配色，未设置的颜色项自动沿用默认值（可选合并，不要求用户写全）。
2. 补上**深浅模式**（跟随系统 / 浅色 / 深色），和上面的配色方案是两个独立维度——选哪套配色、用浅色还是深色版本，可以分别选。
3. 把圆角、间距、阴影、动效收敛成统一的设计 token 体系，参照 Apple Human Interface Guidelines 的专业规范打磨全局细节（间距网格、阴影分级、缓动曲线、语义色的一致性），但**不做字体规范迁移、不做玻璃/半透明材质**——这两项明确排除在外。

## 2. 范围边界

**做的事**：
- 配色方案子系统（内置预设 + 自定义文件，light/dark 两套取值）
- 深浅模式设置（跟随系统 / 浅色 / 深色）
- 圆角设置（单一滑块，全局生效）
- 间距 / 阴影分级 / 动效缓动曲线的 token 化和统一应用
- 现有硬编码节点状态色（`.rect-done` 等）提取成可被配色方案覆盖的 CSS 变量

**明确不做的事**：
- 玻璃/半透明材质（Liquid Glass）——本次讨论过，先放弃
- 自定义字体 / 对齐 SF Pro 的字体规范——继续沿用现有 `font_family`/`font_size` 设置机制，字体不在这次改动范围内
- 任何 Rust 业务逻辑、`build_graph()`、数据库 schema 的改动——这次改动完全在展示层，不涉及任务数据的读写或计算逻辑
- 引入 UI 组件库或切换应用外壳（Tauri → Electron 之类）——评估过，不划算，维持现状

## 3. 配色方案子系统

### 3.1 文件格式：TOML

Key 复用 `style.css` 现有的 CSS 变量名（去掉 `--` 前缀，TOML 的 bare key 本身就允许中划线，不需要额外加引号），外加几个目前是硬编码、这次要提取成变量的节点状态色。完整白名单：

```
bg, bg-dark, bg-panel, bg-select, bg-popup
fg, fg-dim, fg-dark
blue, magenta, cyan, green, yellow, orange, red
border
node-done, node-today, node-overdue, node-locked, node-waiting
```

文件顶层有一个可选的 `name` 字段（方案自己的显示名，设置下拉框里直接用它，不用另外维护一份文件名到显示名的映射），下面按深浅模式分两个 table，两个都可选，table 内每个 key 也都可选：

```toml
name = "沉稳深色系统"

[light]
blue = "#6a4fe0"
node-done = "#e4f5ea"

[dark]
blue = "#8f7dff"
node-done = "rgba(60,190,120,0.16)"
```

没写的 key（包括整个 `[light]`/`[dark]` 都不写）在应用时自动沿用当前生效配色方案的默认值——不是硬编码在 `style.css` 里那套固定默认值，而是"当前选中方案"自己的默认值（细节见 3.4）。

**新增依赖**：`src-tauri/Cargo.toml` 加 `toml`（serde 生态的成熟 crate，纯 Rust 实现，和 `serde_json` 用法几乎一样，`toml::from_str::<T>(s)` 对应 `serde_json::from_str`）。这是本次讨论里第一个真正新增的 Rust 依赖，此前一直优先选择零依赖的方案，这次是用户在了解这个代价后主动选择 TOML 换来的可读性（能写注释、语法对人类更友好）。

### 3.2 内置预设与自定义预设：统一走 Rust 加载

第一版概念稿里的四个方向，处理方式：

- **A（克制中性 + 单一强调色）** 直接成为新的全局默认配色，写进 `style.css` 的 `:root`（浅色）和一个新增的暗色默认块里，不需要单独的方案文件。
- **B（沉稳深色系统）、C（柔和明快）、D（极简黑白灰）** 变成三个内置预设文件：`src-tauri/src/themes/b.toml`、`c.toml`、`d.toml`。

**关键设计决策**：内置预设不再像最初设想的那样由前端直接 `import`——TOML 不是浏览器/Vite 原生能解析的格式，前端引入一个专门解析 TOML 的库不值得（这个项目前端依赖列表一直保持得很精简，没有任何"只为了读一种配置格式"的库）。改成内置预设也用 `include_str!` 编译进 Rust 二进制（跟自定义预设从磁盘读取一样，都在 Rust 里用 `toml::from_str` 解析），通过**同一个** Tauri 命令返回给前端——Tauri 的 IPC 边界本身就是把 Rust 结构体序列化成 JSON 传给前端，所以不管源文件在磁盘上是 TOML 还是别的什么格式，前端拿到手的永远是普通 JS 对象，**前端完全不需要知道、也不需要解析 TOML**。这比最初"内置预设走前端 import、自定义预设走后端命令"两条路径的方案更统一，是这次格式讨论意外带来的简化。

四套的主色调和 node 状态色取值以第一版概念稿 Artifact 里已经定的 token 为准，但**不是直接照抄**——Artifact 用的是简化过的 `--accent`/`--node-*` 命名，真实的 `style.css` 是 7 个各有语义的色相变量（`blue`=交互焦点/选中，`red`=危险/超时，`green`=成功/完成，`yellow`=警告，`orange`=次级提示，`cyan`=图标强调，`magenta`=标签/多选），这 7 个在十几个组件里都在用（已用 grep 核实）。迁移时 Artifact 的 `accent` 对应到 `blue`，`node-*` 直接对应到新的 `node-*` key；`magenta`/`cyan`/`yellow`/`orange`/`green`/`red` 这几个 Artifact 里没有细化到的色相，实施时要按每套方案的色调重新选值（不是简单复用默认方案 A 的），保持同一方案内部协调——这部分是实施阶段的具体调色工作，本文档只定规则、不预先给出全部色值。

内置预设文件（b/c/d.toml）应该给全部 21 个 key 的完整值，不依赖回落到方案 A 的默认值——不然会出现"选了 C 但某个次要颜色其实是 A 的"这种串色。真正利用"没写的 key 用默认值"这个特性的场景是自定义文件：普通用户通常只想改一两个颜色，不需要每次都写全 21 项。

### 3.3 自定义预设：存放、发现与校验

数据目录下新增 `themes/` 子文件夹，和 `settings.json`、`tasks.db` 同级：

```
<数据目录>/
  tasks.db
  settings.json
  themes/
    my-theme.toml
```

Rust 侧用带类型的结构体承接，而不是拿一个原始的 `HashMap`/`Value` 手动挨个检查 key——`#[serde(deny_unknown_fields)]` 直接让"出现白名单之外的 key"在反序列化阶段就失败，报错里自带是哪个字段，比手写校验代码更可靠：

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    pub bg: Option<String>,
    #[serde(rename = "bg-dark")]
    pub bg_dark: Option<String>,
    // ……其余 19 个 key 同样是 Option<String>，短横线命名一律用 #[serde(rename)]
    #[serde(rename = "node-done")]
    pub node_done: Option<String>,
    // ……
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorScheme {
    #[serde(default)]
    pub name: Option<String>,
    pub light: Option<ColorTokens>,
    pub dark: Option<ColorTokens>,
}
```

反序列化成功后，还要再过一遍每个 `Some(String)` 值本身：必须匹配合法的 `#rgb`/`#rrggbb`/`rgba(...)` 语法，否则拒绝并在错误信息里指出是哪个 key 的值不合法（`deny_unknown_fields` 只管字段名对不对，管不了值本身合不合法，这一步不能省）。

新增两个 Tauri 命令（延续 `list_system_fonts` 的套路，纯读取、不碰 `build_graph()`）：

```rust
/// 列出所有可选配色方案（内置 3 套 + 数据目录 themes/ 下发现的自定义文件），
/// 每项带一个可以直接显示的名字（内置的是写死的中文名，自定义的读文件里的
/// name 字段，没写就用文件名兜底）
#[tauri::command]
pub fn list_color_schemes() -> Vec<ColorSchemeInfo> // { id: String, name: String }

/// 按 id 加载一个配色方案（"builtin:b/c/d" 走 include_str! 内嵌内容，
/// "custom:<filename>" 走磁盘读取），统一校验后返回
#[tauri::command]
pub fn get_color_scheme(id: String) -> Result<ColorScheme, String>
```

`get_color_scheme` 对 `"custom:<filename>"` 的额外校验：`filename` 不允许包含路径分隔符或 `..`（防止路径穿越读到 `themes/` 目录之外的文件）。

设置弹窗（`SettingsModal.vue`）每次打开时调用一次 `list_color_schemes()` 刷新下拉框选项，不需要额外的"刷新"按钮——这类本地文件的增删本来就不频繁，弹窗重新打开就足够及时。

### 3.4 应用逻辑（前端）

由于 3.2 的决策，前端拿到的配色方案数据（不管内置还是自定义）都是同一种形状的普通对象（`{ name?, light?, dark? }`），处理逻辑完全不用区分来源。`App.vue` 按现有 `applyFontSize`/`applyFontFamily` 的套路，新增：

```js
function applyColorScheme(scheme, effectiveMode) {
    // 关键：先清空全部 21 个 key 的内联覆盖，再按需要设置——
    // 否则从"方案 B"切到"方案 D"时，B 设置过、D 没设置的 key 会保留 B 的内联值，
    // 不会正确回落到 style.css 里方案 A 的默认值。这一步不能省。
    for (const key of COLOR_TOKEN_KEYS) {
        document.documentElement.style.removeProperty(`--${key}`);
    }
    const tokens = scheme?.[effectiveMode] ?? {};
    for (const [key, value] of Object.entries(tokens)) {
        if (value == null) continue;
        document.documentElement.style.setProperty(`--${key}`, value);
    }
}

function applyCornerRadius(px) {
    document.documentElement.style.setProperty("--app-radius", `${px}px`);
}
```

`effectiveMode`（当前到底该用浅色还是深色）不靠 CSS 的 `prefers-color-scheme` 媒体查询，而是 JS 自己算出来、再整体切换：

```js
function computeEffectiveMode(themeMode) {
    if (themeMode === "light" || themeMode === "dark") return themeMode;
    // themeMode === "system"
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
```

`style.css` 里对应新增一个 `:root[data-theme-mode="dark"]` 块（A 方案的暗色默认值），和现有 `:root`（A 方案的浅色默认值）并列。`App.vue` 在 `computeEffectiveMode` 算出结果后，先 `document.documentElement.setAttribute("data-theme-mode", effectiveMode)` 切换这个属性——这一步单靠 CSS 层叠就能让全部 21 个 token 在"方案 A 的浅色默认值"和"方案 A 的暗色默认值"之间整体切换，不需要在 JS 里额外备份一份默认值表；然后如果 `color_scheme` 不是默认值，才调用 `applyColorScheme()` 在这个已经确定深浅的基础上叠加内联覆盖。这样默认值只有 `style.css` 一份数据源，JS 不用维护重复的一份，不存在"两边改一个忘了改另一个"的风险。

`onMounted` 里加载完 settings 后，依次调用一次上面这套流程和 `applyCornerRadius(...)`；`saveSettings()` 保存成功后同样重新跑一遍（跟现有字体设置的处理方式一致）。另外注册一个 `matchMedia("(prefers-color-scheme: dark)")` 的 `change` 监听：只有当 `theme_mode === "system"` 时才需要响应，触发时重新走一遍上面的流程，这样用户切系统主题时应用能跟着实时变，不用重启。

自定义方案如果在加载时读取失败（文件被删了、内容不合法），前端直接按"未设置自定义方案"处理，回落到内置默认，并给一次性的错误提示——不让整个应用因为一个坏配色文件起不来，这跟现有 `settings::load()` "解析失败就返回默认值"的容错思路是一致的。

### 3.5 Settings 新增字段（`src-tauri/src/settings.rs`）

```rust
/// 配色方案："" 表示内置默认（A）；"builtin:b"/"builtin:c"/"builtin:d" 表示内置预设；
/// "custom:<filename>" 表示 themes/ 目录下的自定义文件
#[serde(default)]
pub color_scheme: String,

/// 深浅模式："system" | "light" | "dark"
#[serde(default = "default_theme_mode")]
pub theme_mode: String,

/// 全局圆角基准（像素），派生出 --radius-sm/md/lg 三档
#[serde(default = "default_corner_radius")]
pub corner_radius: u32,
```

默认值：`default_theme_mode() = "light"`（不用 `"system"`——现有用户升级后如果系统本身开着深色模式，不应该让应用外观在没有明确操作的情况下突变，深浅模式是要用户自己选的）；`default_corner_radius() = 10`。

`save_settings` 校验分支：
- `theme_mode` 必须是 `"system"`/`"light"`/`"dark"` 三者之一
- `corner_radius` 限制在合理范围（比如 0–24）
- `color_scheme` 格式校验：允许空串，或 `builtin:b|c|d`，或 `custom:<不含路径分隔符的文件名>`；具体某个自定义文件是否存在/合法，交给 3.3 的 `get_color_scheme` 在实际应用时校验，这里只做格式层面的校验

## 4. 圆角 token 化

新增 `--app-radius`（用户可调，默认 10px）以及三档派生 token：

```css
--radius-sm: calc(var(--app-radius) * 0.6);   /* 默认 6px，对应现在 input 的 6px */
--radius-md: var(--app-radius);                /* 默认 10px */
--radius-lg: calc(var(--app-radius) * 1.4);   /* 默认 14px */
```

逐个替换掉现在散落在各组件 `<style scoped>` 里的硬编码 `border-radius`（按视觉量级归到三档之一），以及 `TaskGraph.vue` D3 节点矩形的圆角（目前节点矩形没有显式设置 `rx`，属于要新增的一部分，让节点也跟随圆角设置）。

## 5. 全局视觉打磨（Apple HIG 取向，不含字体/玻璃）

- **间距**：新增一套 4px 基准的间距 token（`--space-1: 4px` 到 `--space-8: 32px` 左右），组件里逐步替换随手写的 margin/padding 数值——这一项工作量大、优先级低，本次按"顺手改到的地方就用新 token，不强求一次性扫全部组件"处理，不阻塞其它项。
- **阴影分级**：收敛成 2–3 级 `--elevation-1`/`--elevation-2`/`--elevation-3`（对应静止面板、悬浮态、弹出层/Modal），替换现在各处力度不一的 `box-shadow`/`filter: drop-shadow(...)`。
- **动效**：定义 1–2 条标准缓动曲线（一条常规 `--ease-standard: cubic-bezier(0.4, 0, 0.2, 1)`，一条类 spring 的 `--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1)` 用于弹窗/悬浮层出现），替换现在参差不齐的 `transition: opacity 0.25s`/`0.15s` 散值。
- **语义色**：`node-done`/`node-today`/`node-overdue`/`node-locked`/`node-waiting` 在 A 方案 light/dark 下的具体取值，沿用第一版概念稿已经定的色值（浅色是不透明淡色块，深色是低不透明度的彩色叠加，参考 Artifact 里 `[data-style="a"]` 的定义）。

## 6. 需要改动的文件（供后续实施计划参考）

**后端**：
- `src-tauri/Cargo.toml` —— 新增 `toml` 依赖
- `src-tauri/src/settings.rs` —— 新增 `color_scheme`/`theme_mode`/`corner_radius` 三个字段、默认值函数、文档注释
- `src-tauri/src/themes/b.toml`、`c.toml`、`d.toml` —— 新建，内置预设，`include_str!` 嵌入二进制
- `src-tauri/src/commands.rs`（或新增一个 `src-tauri/src/color_scheme.rs` 模块，`ColorTokens`/`ColorScheme`/`ColorSchemeInfo` 结构体和校验逻辑放这里更合适，`commands.rs` 只放 `#[tauri::command]` 薄封装）—— 新增 `list_color_schemes`/`get_color_scheme` 命令；`save_settings` 加对应校验分支
- Tauri 命令注册处（`invoke_handler![...]` 列表所在文件）—— 注册两个新命令

**前端**：
- `frontend/src/style.css` —— `:root` 换成 A 方案的浅色值；新增 `:root[data-theme-mode="dark"]` 暗色默认块；`.rect-*` 系列改成引用新的 `node-*` CSS 变量；新增 `--app-radius`/`--radius-*`/间距/`--elevation-*`/`--ease-*` 一系列 token；组件外的全局样式（D3 动态元素那节）里的圆角/阴影/动效改用新 token
- `frontend/src/composables/useApi.js` —— 新增 `listColorSchemes()`、`getColorScheme(id)` 封装（注意参数按 camelCase 传）
- `frontend/src/App.vue` —— `settings` ref 新增三个字段；新增 `applyColorScheme`/`applyCornerRadius`/`computeEffectiveMode`；`matchMedia` 监听；`onMounted` 和保存设置后调用
- `frontend/src/components/SettingsModal.vue` —— 新增"外观"分区：配色方案下拉、深浅模式选择、圆角滑块；打开弹窗时回填这三项 + 刷新自定义配色列表
- 各组件 `<style scoped>` 里硬编码 border-radius/box-shadow/transition 的地方逐个换成新 token（`TaskFormModal.vue`、`ConfirmDialog.vue`、`TagManagerModal.vue`、`TimeEntryNoteModal.vue`、`IconPicker.vue`、`DatePicker.vue`、`ProjectTree.vue`、`ProjectTreeNode.vue`、`ProjectContextMenu.vue`、`Dashboard.vue`、`TaskDetail.vue`、`TaskListView.vue`、`ChartsPage.vue`、`CalendarPage.vue`、`ColorSwatchPicker.vue`）—— 这部分是"全局打磨"里工作量最大的一块
- `frontend/src/components/TaskGraph.vue` —— D3 节点矩形新增 `rx` 绑定到 `--radius-md`

## 7. 验证方式

- `cargo check`：后端类型检查
- `cargo test`：确认没有意外影响到现有测试（这次改动不涉及被测的业务逻辑，预期全部照常通过）
- `pnpm run build`（`frontend/`）：捕获模板/脚本错误
- 用 `run` 技能把应用实际跑起来，人工核对：
  - 默认启动即为 A 方案浅色配色
  - 设置里切换 B/C/D 立即生效，切换深浅模式立即生效，且"跟随系统"下手动切换系统主题应用能跟着变
  - 拖动圆角滑块，按钮/输入框/卡片/图节点的圆角同步变化
  - 把一个只写了部分 key 的自定义 TOML 放进 `themes/` 目录，设置弹窗能选到，未写的 key 正确回落默认值
  - 放一个包含未知 key 或非法颜色值的自定义文件，选中时报错清晰，不影响应用其它部分正常使用

## 8. 需要同步的变更日志

按项目约定，实施完成后在 `CHANGELOG.md`（如果还没有则新建）的"未发布"章节补充条目，具体到本文档第 6 节列出的文件/函数/设置项级别。
