# CLAUDE.md

本文件为 Claude Code（claude.ai/code）在本仓库中工作时提供指导。

## 项目简介

一个本地优先的桌面任务管理工具（Tauri 2 + Rust 后端，Vue 3 前端），以 DAG（有向无环图）可视化任务依赖关系。不依赖云服务、不依赖网络、不依赖 Taskwarrior——所有数据都存在本地 SQLite 文件里。仓库目录名是 `task-web`，但实际的产品/包名是 **`task-graph`**（见 `src-tauri/tauri.conf.json` 里的 `productName`/`identifier`、crate 名 `task-graph`、前端包名 `task-graph-frontend`）——不要被这个不一致搞混。

项目早期有一个 Python + Taskwarrior + FastAPI 的网页版，已经完全删除；现在只有 Tauri 桌面应用这一个版本。

## 常用命令

```bash
# 安装前端依赖（首次 / 拉取到依赖变更后）
cd frontend && npm install

# 开发模式：同时拉起 Vite 和 Tauri 窗口，支持热重载
cd src-tauri && cargo tauri dev
# （cargo tauri dev 会根据 tauri.conf.json 的 beforeDevCommand 自动执行
#  `npm run dev --prefix frontend`，不需要单独再起一个 Vite）

# 生产构建（产物在 src-tauri/target/release/bundle/ 下，按平台分类）
cd src-tauri && cargo tauri build

# Rust：只做类型检查/编译检查（快，改后端代码时优先用这个）
cd src-tauri && cargo check

# Rust：跑后端测试（schema 迁移测试、重复任务日期计算测试、图可达性测试）
cd src-tauri && cargo test
cd src-tauri && cargo test <test_name>   # 只跑单个测试

# 前端：只有生产构建，package.json 里没有配置专门的 lint/test 脚本
cd frontend && npm run build
```

前端没有测试套件，也没有配置 lint 命令——不要臆造 `npm test`/`npm run lint` 这类命令。验证前端改动用 `npm run build`（能捕获 Vue 模板/脚本错误），条件允许的话直接把应用跑起来验证。

注意：`frontend/vite.config.ts` 是脚手架遗留的杂物文件（引用了 `vite-plugin-vue-devtools`，但这个包根本不在依赖列表里；还配了一个指向已废弃 FastAPI 后端的 `/api` 代理）。真正生效的是 `frontend/vite.config.js`（两个同时存在时 Vite 优先用 `.js`）——要改配置改这个文件，不要改 `.ts` 那个。

## 数据存储与便携性

`tasks.db` 和 `settings.json` 默认保存在可执行文件所在目录旁边（如果这个目录不可写，会回退到一个类似 XDG 规范的数据目录，见 `src-tauri/src/db/mod.rs`），不会写入 `~/.local/share` 等系统标准路径。这是有意为之：把程序文件夹拷到任意位置（包括 U 盘）、删除文件夹，就是完整的安装/卸载流程。开发/测试时可以用 `TASK_GRAPH_DATA` 环境变量覆盖数据库路径。

设置**有意**存成独立的 `settings.json`，而不是 SQLite 里的一张表（见 `src-tauri/src/settings.rs` 顶部的文档注释）——这样用户偏好可以脱离任务数据单独导出/同步，也不用让偏好设置的改动跟着数据库的迁移生命周期走。以后新增任何全局性的应用偏好都follow这个先例；只有那种带关系型结构、写入频繁的数据才放进 SQLite。

## 后端架构（`src-tauri/src/`）

**几乎所有 Tauri 命令最终都会汇入 `commands.rs` 里的 `build_graph()`。** 新建/修改/删除任务、计时开始停止、项目移动、标签编辑、重复规则变更等命令，几乎都是在最后调用 `build_graph()`，把*完整的*图数据当作 `GraphResponse` 整体返回——没有增量/局部更新协议。前端每次都是用拿到的 `GraphResponse` 整体替换本地状态（见 `App.vue` 里的 `applyUpdate()`）。新增一个会改数据的命令时，遵循这个套路：打开连接 → 把实际 SQL 操作委托给某个 `db::<module>::<fn>` → 调用 `build_graph()` 并返回其结果。

`build_graph()` 本身按顺序做这些事：

1. `db::project::purge_expired` —— 自动清理超过保留期的废纸篓项目
2. `db::task::reset_stale_today_marks` —— 清除跨天失效的"今日任务"标记
3. `db::recur::process_rollovers` —— 给已经过了周期的重复任务"补课"（见下文）
4. `db::task::list_all` + `apply_derived_fields` —— 加载任务，计算所有非直接存储的字段
5. 构建 `edges`（从 `depends` 来）和 `today_order_edges`（独立的手动排序图）、项目树、标签字典

这是一个**没有后台常驻进程/定时任务**的桌面应用。所有跟时间相关的逻辑（今日标记过期、重复任务周期翻篇、废纸篓自动清理）都是在每次调用 `build_graph()` 时惰性"补课"，而不是靠定时器触发——以后加任何跟时间相关的新功能，延续这个模式，不要引入调度器。

**`apply_derived_fields()`** 是唯一的汇合点，所有计算出来（非数据库列直接存储）的 `Task` 字段都在这里、且只在这里填充：`is_overdue`、`is_due_today`、`is_today`、`is_locked`、`blocking`、`total_seconds`、`is_timing`、`active_since`。新增一个派生字段时：先在 `models/task.rs` 的 `Task` 结构体里加字段（带 `#[serde(default)]`，避免旧数据/部分字段反序列化失败），在 `db/task.rs` 的 `row_to_task()` 里给个默认值，然后在 `apply_derived_fields()` 里真正算出它的值。

**Schema 迁移**（`db/schema.rs`）是一个纯 `&[&str]` 数组，只追加、不修改，由 `schema::init()` 自动执行并记录版本号（`schema_version` 表）。永远不要改已有的条目，只能追加新的。如果某次迁移需要搬迁/转换已有数据（不只是建表这种 DDL），就在 `init()` 的迁移循环里加一个针对该版本号的特殊分支处理（参考版本 9 后紧跟着执行的"标签从 JSON 搬到关系表"的回填逻辑，必须赶在版本 10 删掉旧列之前完成）。

**重复任务**（`db/recur.rs` + `models/recur.rs`）：一个重复任务是*同一条* `Task` 记录在待办/完成之间循环，而不是每个周期都生成一条新记录。`models/recur.rs` 是纯粹的日期计算（`next_cycle_due`、`first_cycle_due`，对应 `RecurRule::{Daily,Weekly,Monthly}`）。`db/recur.rs::process_rollovers()` 会遍历所有设了 `recur_rule` 的任务，把已经完全过去的周期结算进 `recur_log`（`completed_on_time` / `completed_late` / `missed`），并把任务重置为待办、进入新周期。连续多个错过的周期会被折叠成一条 `missed` 记录（省事，且对"连续天数"计算无损——那个计算只是按 `cycle_due DESC` 扫描 `recur_log`，遇到第一条 `missed` 就停）。所有周期截止时间统一是 `23:59:59Z`——和这个应用其它地方一样，不做任何用户时区处理，约定一律用 UTC。

**"今日排序"图**（`db/today_order.rs` + `graph_utils.rs`）："今日任务"视图允许用户在标记为 `is_today` 的任务之间手动画一张*独立*的排序图（`today_order_edges` 表），跟真实的 `depends` 依赖图无关。`graph_utils.rs` 提供纯粹的 BFS 可达性判断（`forward_adjacency`、`reachable`），用来校验新加的手动边不会跟真实依赖方向矛盾，也不会在手动排序图内部形成环。这个模块特意跟 `apply_derived_fields()` 分开——它是 `add_today_order_edge` 里按需调用的"写入前校验"，不是每次读取都要跑的投影逻辑。

**项目树**（`db/project.rs` + `models/project.rs`）：项目分类不是一个严格的枚举标签。`ProjectNode` 有几个各自独立、会级联的维度——`stage`（`"planned"`/`"active"`，只能在顶层项目上设置，子项目继承）、`archived`、`trashed`（配合 `trashed_at` 驱动自动清理）。一个项目最终落在 UI 上的哪个分组（废纸篓 > 已归档 > 阶段，按此优先级）由 `db/project.rs` 里的 `group_of()` 算出，并写进每个 `ProjectNode.group` 字段——前端直接读这个字段，不需要重新实现一遍优先级判断逻辑。`db/project.rs::build()` 还处理"无项目"这个虚拟桶，并且只要某个子树的有效分组跟父级不一样（比如某个子项目被单独归档），就把它从父级的 children 里摘出来，作为所属分组自己的根节点。

**Emoji 校验**：`commands.rs` 里的 `validate_icon()` 用 `unicode-segmentation` crate 按 grapheme cluster（字形簇）计数，而不是 `chars().count()`——旗帜、肤色变体、ZWJ 家庭组合 emoji 这类是一个视觉字符但对应多个 Rust `char`。

## 前端架构（`frontend/src/`）

**靠 `v-show` 切换页面，不用路由。** `App.vue` 的 `currentPage` ref（取值 `"home" | "board" | "charts" | "calendar"`）控制哪个顶层页面组件可见；所有页面组件其实都常驻挂载（这样比如标题栏的计时器状态才能在任意页面下都正常工作）。页面组件通常会接一个 `:visible="currentPage === 'x'"` prop，这样切回这一页时知道要重新拉一次数据，而不需要真的被卸载/重新挂载。

**`useApi.js`** 是唯一调用 `invoke()` 的地方。每个封装函数传的参数对象，key 必须是**驼峰命名（camelCase）**，这是为了匹配 Tauri v2 对"直接参数（非 struct 包裹）"命令默认的 IPC 参数命名规则——比如 Rust 端 `add_today_order_edge(from_uuid, to_uuid)`，JS 端要传 `{ fromUuid, toUuid }`，*不是* `{ from_uuid, to_uuid }`。这个坑已经导致过真实 bug（`rename_tag` 的封装函数曾经因为这个原因悄悄失效，后来才修）。唯一的例外：Rust 签名里只接收一个 struct 参数的命令（比如 `ReconnectDependencyArgs`），调用时要传 `{ args: { 蛇形命名字段... } }`——这种情况下 serde 匹配的是 struct 自身的字段名，不受 IPC 驼峰命名规则约束，所以这些字段保持蛇形命名，跟 Rust struct 定义保持一致。

**`useLayout.js`** 是纯图/布局逻辑，不碰 DOM/D3：`computeLayout()`（dagre 布局 + 项目/标签过滤）、`computeHighlight()`（基于 BFS 算祖先/后代/邻居，供"链路高亮"功能用，也被今日排序视图的链路预览叠加层复用）、`wouldCreateCycle()`（在依赖关系改动真正发到后端之前，前端先做一次环检测预判）。

**`TaskGraph.vue`** 是用 D3 渲染的 DAG 图。需要知道的关键点：它有个 `mode` prop（默认 `"depends"`，或 `"today-order"`）——`today-order` 模式下，传给它的 `edges` prop 实际上是 `today_order_edges`（由 `App.vue` 负责切换，TaskGraph 自己不做这个判断），另有一个独立的 `dependsEdges` prop 始终携带真实依赖边，用于在悬浮/选中某个任务时渲染一层虚线的"这个任务实际依赖什么"预览（`renderChainOverlay()`，一个跟主 `render()` 分开的轻量级 D3 数据绑定，避免悬浮时触发整体重新渲染）。D3 动态插入的 SVG 元素（节点、边、箭头）用不了 Vue 的 `<style scoped>`——它们的样式统一放在全局的 `frontend/src/style.css` 里的"D3 动态元素样式"那一节；新增一个 D3 动态生成的 CSS 类，样式规则也要写在那里，不要写进某个组件的 `<style scoped>`。

**项目/分类/今日筛选**（`App.vue` 里的 `selectedProject`，往下作为 `projectFilter` 传给 `TaskGraph`/`useLayout`）是同一个字符串变量，靠 `config/constants.js` 里定义的几种哨兵值区分含义：真实的项目路径、`INBOX_PROJECT`（"无项目"）、`stageFilter(group)` 拼出来的值（如 `"__stage__planned"`，对应项目树里四个可点击的分类标题）、或 `TODAY_PROJECT`（`"__today__"`）。`useLayout.js::filterNodes()` 会按拿到的是哪种值分支处理。凡是要从 `selectedProject` 派生别的东西（比如"新建任务表单要默认填哪个项目"），记得把这三种哨兵值都排除掉，不能只判断是不是 `null`。

**设置的数据流**：`App.vue` 持有一个本地 `settings` ref，镜像后端的 `Settings` 结构体，通过 `getSettings()` 加载一次、通过 `setDurationFormat()`/`applyFontSize()` 应用生效（字号设置实际生效的方式是一个 CSS 自定义属性 `--app-font-size`），每次保存后重新应用一遍。新增一个设置字段，需要在三处同步加默认值：Rust 端 `Settings::default()`/对应的 `default_xxx()` 函数、`App.vue` 里本地 `settings` ref 的初始值、`SettingsModal.vue` 打开弹窗时回填数据的 watcher。

## 值得延续的固定套路

- 新 Tauri 命令 → `Args` 结构体（可选字段带 `#[serde(default)]`）→ 委托给 `db::` → `build_graph()` → 用 `.map_err(|e| e.to_string())` 转成 `Result<GraphResponse, String>` 返回。
- 新的派生 `Task` 字段 → struct 加字段 + `row_to_task()` 给默认值 + `apply_derived_fields()` 里算出实际值。
- 新增数据库表/列 → 追加到 `schema.rs` 的 `MIGRATIONS` 数组里，永远不改已有的条目。
- 新的设置项 → 放进独立的 `settings.json`，不放 SQLite，除非它是关系型/高频写入的数据。
- 新的顶层页面 → 在 `App.vue` 里加一个 `currentPage` 取值 + 导航按钮 + `v-show` 区块 + `:visible` prop。
