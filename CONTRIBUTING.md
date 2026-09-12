# 贡献指南

感谢你对 task-graph 感兴趣！在提交 Issue 或 Pull Request 前，请花几分钟阅读本文档。

## 开发环境

```bash
# 安装前端依赖
cd frontend && pnpm install

# 开发模式（Vite + Tauri 窗口，支持热重载）
cd src-tauri && cargo tauri dev
```

更多命令（生产构建、测试等）见 [README.md](README.md)。

## 提交 Issue

- Bug 报告请说明操作系统、复现步骤、预期行为与实际行为，尽量附截图或日志。
- 功能建议请说明使用场景，方便判断是否符合项目定位（本地优先、不依赖云服务/网络）。

## 提交 Pull Request

1. Fork 本仓库，基于 `master` 新建分支。
2. 改动前端后请至少跑一次 `cd frontend && pnpm run build` 确认无构建错误（前端未配置 lint/test）。
3. 改动 Rust 后端后请跑：
   ```bash
   cd src-tauri && cargo check
   cd src-tauri && cargo test
   ```
4. 遵循仓库已有的代码风格和架构约定（详见 [CLAUDE.md](CLAUDE.md)），例如：
   - 新增 Tauri 命令统一走 `Args` 结构体 → 委托给 `db::` 模块 → 调用 `build_graph()` 返回完整图数据。
   - 新增派生 `Task` 字段需要同步修改 `models/task.rs`、`db/task.rs::row_to_task()`、`apply_derived_fields()` 三处。
   - 数据库 schema 变更只能追加到 `schema.rs` 的 `MIGRATIONS` 数组，不能修改已有条目。
   - 涉及"今天/这一天"的判断一律使用本地时区（后端 `chrono::Local`，前端 `useLocalTime.js`），存储仍用 UTC。
5. Commit message 建议使用简洁的中文描述，说明改动的原因而非罗列改了哪些文件。
6. 提交 PR 时请描述改动动机、测试方式，如涉及 UI 改动请附截图。

## 许可证

提交 PR 即表示你同意你的贡献以 [GPL-3.0](LICENSE) 协议发布。
