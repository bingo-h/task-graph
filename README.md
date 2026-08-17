# task-graph

本地优先的桌面任务管理工具，以 DAG（有向无环图）可视化任务依赖关系。

项目层级作为过滤维度，支持高亮任务链路、查看任务详情、图形化新建/修改任务。数据完全存储在本地 SQLite，不依赖 Taskwarrior，不依赖任何云服务，不依赖网络。

## TODO

- [ ] 月度记录 Monthly Log
- [ ] 单个任务类 git 分支记录

## 界面结构

![预览图片](preview/preview.png)

## 功能特性

- **DAG 可视化**：任务依赖关系以有向图展示，从左到右布局
- **锁定状态**：前置任务未完成时显示 🔒，节点颜色变暗
- **高亮模式**（可切换）：
  - 祖先链路（默认）：高亮从根到当前节点的完整链路
  - 直接上下游：只高亮直接前置和后续任务
  - 完整链路：高亮选中节点所在的整条链路（含后续）
- **项目树**：左侧显示项目层级，点击过滤 DAG 图，含进度条和逾期警告
- **任务详情**：右侧显示完整元数据，支持完成/取消完成/修改/删除
- **任务计时**：详情栏一键开始/停止计时，累计耗时实时显示；同一时刻全局只允许一个任务计时，开始新计时会自动结束上一个；计时明细按日期分组，可查看每一段具体的起止时间
- **图形化表单**：新建/修改任务用表单操作，无需记忆任何命令语法
- **平移缩放**：鼠标拖拽平移，滚轮缩放，⊙ 按钮重置视图
- **便携式数据存储**：数据库文件保存在程序自身所在目录，不写系统任何位置
- **重复任务（习惯打卡）**：任务可设置每 N 天 / 每周选星期几 / 每月选日期的重复规则；到期自动重置为待办并加入"今日任务"，逾期未完成会在图中标红，直到下一个周期到来才重置；每个周期的完成结果（按时/逾期补做/错过）都会记录，用于计算连续完成天数
- **今日任务分类**：项目导航里独立一个"今日任务"入口，汇总所有标记为今日的任务，不受实际项目归属限制；这个视图下依赖图边替换成用户手动拖连的"今日工作顺序"，与真实依赖关系解耦但不能违反其先后约束；悬浮/选中任务可虚线预览它在真实依赖图里的完整链路
- **项目分类导航**：计划中 / 进行中 / 已归档 / 回收站四个分组标题本身可点击，直接筛选该分类下所有任务
- **日历页**：月视图展示每个重复任务的打卡记录（图标/圆点）与连续天数；点击某一天进入日视图，按时间轴展示当天各时段花在哪些任务上
- **分析页**：今日任务耗时（横向柱状图，可按任务/项目汇总）、任务完成趋势（柱状图/折线图可切换）

## 技术栈

- 桌面框架：Tauri 2（Rust 后端 + 系统 WebView，非 Electron）
- 数据层：SQLite（rusqlite，本地存储，无外部依赖）
- 前端：Vue 3 + D3.js + dagre

> 本项目早期版本基于 Python + Taskwarrior + FastAPI 实现网页版，已完全弃用。当前是 Tauri 桌面应用。

## 环境要求（开发环境）

- Rust 1.75+
- Node.js 18+
- Tauri CLI：`cargo install tauri-cli --version "^2"` 或用 `npm run tauri`（通过 devDependencies 中的 `@tauri-apps/cli`）

## 开发环境启动

```bash
# 安装前端依赖（含 @tauri-apps/api、@tauri-apps/cli）
cd frontend && npm install && cd ..

# 启动开发模式（自动拉起 Vite + Tauri 窗口，支持热重载）
cd src-tauri
cargo tauri dev
```

`cargo tauri dev` 会根据 `tauri.conf.json` 里的 `beforeDevCommand` 自动启动
`frontend` 目录下的 `npm run dev`，不需要手动开两个终端。

## 生产构建

```bash
cd src-tauri
cargo tauri build
```

构建产物位置（因平台而异）：

- Linux：`src-tauri/target/release/bundle/appimage/*.AppImage`、`bundle/deb/*.deb`
- Windows：`src-tauri/target/release/bundle/nsis/*.exe`、`bundle/msi/*.msi`

## 数据存储

数据库文件 `tasks.db` 保存在**可执行文件自身所在目录**，不写入系统标准路径（不使用 `~/.local/share`、`%APPDATA%` 等）。

设计目的：便携式部署——把程序文件夹拷贝到任意位置（包括 U 盘）、删除文件夹即完全卸载，不留系统残留。

> 注意：macOS `.app` 包和 Linux `.deb` 安装后，可执行文件路径在包内部较深层级，此时"程序自身目录"不完全等同于用户直觉认知的安装目录。若后续需要在这些打包形态下也保持严格便携语义，需改用 Tauri 的 `path().app_data_dir()` API，详见 `src-tauri/src/db/mod.rs` 顶部注释。

可通过 `TASK_GRAPH_DATA` 环境变量覆盖默认路径（主要用于开发调试）。

## 项目结构

按 Tauri 2 官方推荐的标准布局：

```text
task-graph/
├── src-tauri/                 # Tauri 后端（Rust）
│   ├── Cargo.toml
│   ├── tauri.conf.json        # 窗口、打包、构建命令配置
│   ├── build.rs
│   ├── icons/                 # 各平台图标
│   └── src/
│       ├── main.rs            # 薄入口，仅调用 lib.rs 的 run()
│       ├── lib.rs             # 应用逻辑入口，注册所有 Tauri command
│       ├── commands.rs        # Tauri 命令层（前端 invoke() 的对应实现）
│       ├── graph_utils.rs     # 纯图算法（可达性判断），供"今日任务"手动排序校验用
│       ├── settings.rs        # 应用设置读写（独立 settings.json，不放进 SQLite）
│       ├── db/                # SQLite 数据层
│       │   ├── mod.rs             # 连接管理，数据库路径解析
│       │   ├── schema.rs          # 建表与迁移
│       │   ├── task.rs            # 任务 CRUD
│       │   ├── project.rs         # 项目树构建
│       │   ├── time_entry.rs      # 任务计时记录 CRUD
│       │   ├── recur.rs           # 周期性任务回滚算法 + 完成记录读写
│       │   └── today_order.rs     # "今日任务"手动排序边 CRUD
│       └── models/            # 数据结构
│           ├── task.rs            # Task 结构体 / RecurRule 枚举
│           ├── project.rs         # ProjectNode 结构体
│           ├── urgency.rs         # 紧迫度计算公式
│           ├── recur.rs           # 周期性任务的日期计算（纯函数）
│           └── time_entry.rs      # TimeEntry 结构体
└── frontend/                  # 前端（Vue，作为 Tauri 的 WebView 内容）
    ├── package.json
    ├── vite.config.js
    └── src/
        ├── App.vue                    # 根组件
        ├── style.css                  # 全局样式
        ├── config/constants.js        # 全局常量
        ├── components/
        │   ├── Dashboard.vue          # 首页仪表盘
        │   ├── ChartsPage.vue         # 分析页
        │   ├── CalendarPage.vue       # 日历页（月视图打卡 + 日视图时段回顾）
        │   ├── ProjectTree.vue        # 项目树容器
        │   ├── ProjectTreeNode.vue    # 项目树递归节点
        │   ├── TaskGraph.vue          # D3 DAG 图
        │   ├── TaskDetail.vue         # 任务详情面板
        │   ├── TaskFormModal.vue      # 新建/修改任务表单
        │   └── SettingsModal.vue      # 设置弹窗
        └── composables/
            ├── useApi.js              # 数据请求封装（Tauri invoke）
            ├── useLayout.js           # dagre 布局、链路高亮、依赖环检测
            ├── useDuration.js         # 时长格式化
            └── useTagColor.js         # 标签/任务颜色小工具
```
