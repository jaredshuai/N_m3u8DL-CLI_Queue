# 项目上下文

## 这个项目是做什么的

一个 m3u8 视频队列下载器。真正的下载由命令行工具 N_m3u8DL-CLI 完成，这个项目给它套了一层桌面界面（Tauri + Svelte），提供排队、自动重试、下载历史等功能。

## 技术栈

- **后端**：Rust（在 `src-tauri/` 下），分五层，每层职责隔离、互不越界（细节见 `docs/adr/0001-*.md`）
- **前端**：Svelte + Vite（在 `src/` 下），通过 Tauri 的进程间通信调用后端命令，不是网页那种 HTTP 请求
- **下载内核**：N_m3u8DL-CLI v3.0.2（一个 .NET 程序，作为外部子进程启动）+ ffmpeg
- **旧版 CLI**：`cli/N_m3u8DL-CLI/` 是更早的 .NET 命令行版本（来自上游 nilaoda，已归拢进 `cli/` 目录），目前**冻结**，不再日常维护

## 核心概念

读代码前先了解这些概念，能少走弯路。

| 概念 | 是什么 |
|---|---|
| **任务（Task）** | 一个待下载的 m3u8 链接，带 id、url、保存名、请求头、状态等字段。代码里是 `domain::task::Task` |
| **任务状态** | 五种：`Waiting`（等待中）、`Downloading`（下载中）、`Completed`（完成）、`Failed`（失败）、`Cancelled`（用户停止）。**注意**：同一个状态在每一层有各自的类型，详见 `docs/adr/0003-*.md` 和 `docs/adr/0009-*.md` |
| **队列（Queue）** | 一组排好序的任务，串行处理（同一时间只下载一个） |
| **活跃任务** | 还没结束的任务——也就是"等待中"或"下载中"的。代码里叫 `is_live_work`，用来判断队列是不是"还活着" |
| **停止任务（Stop Task）** | 用户主动终止 `Downloading` 子进程，把任务转为 `Cancelled`。它可删除、可手动重试，但不走失败自动重试、不进入历史，也不触发自动关机倒计时。详见 `docs/adr/0009-*.md` |
| **历史（History）** | 已经到头的完成或失败任务留下的不可变记录；`Cancelled` 当前不进入历史 |
| **产物（Artifact）** | 下载完成后落在磁盘上的视频文件。ADR-0005 采纳后目标类型是 `ArtifactPath`（application 层 newtype）。详见 `docs/adr/0005-*.md` |
| **产物目录（Artifact Dir）** | 产物所在目录，ADR-0005 目标类型 `ArtifactDir`（application 层 newtype，absolute non-canonical）。和 `ArtifactPath` 配对：dir 是输入（要盘点的目录），path 是输出（找到的产物路径） |
| **产物定位（Artifact Location）** | 下载子进程成功退出后，在下载目录里找到产物文件路径的业务策略——扩展名白名单、save_name 精确匹配、save_name 前缀匹配、新鲜度窗口、mtime 排序。策略是 application 层的纯函数 `locate_artifact`，文件系统访问走 `ArtifactInventory` port |
| **产物盘点（Artifact Inventory）** | adapter 对下载目录做的一次只读快照，返回 `ArtifactDirectorySnapshot`（含 presence/entries/skipped_entry_count）。port 只报事实，不做策略判断 |
| **产物解析（Artifact Resolution）** | application 收到"子进程完成"信号后，调用产物盘点 + 定位策略得到的三态结果：`Located(path)` / `NotFound` / `InventoryUnavailable(err)`。是 application 内部类型（不进事件 payload），由 `handle_completed_child_exit` 算出后投影到 `TaskSnapshot` |
| **产物诊断（Artifact Diagnostic）** | 当产物解析为 `InventoryUnavailable` 时，随 `TaskSnapshot` 持久化的诊断信息（kind + message），解释 `output_path: None` 的原因。前端暂不展示 |
| **端口（Port）** | 后端内部的一种约定（interface）：某一层声明"我需要别人提供什么能力"，比如"我需要一个时钟""我需要能存任务的仓库"。代码里是 `src/ports/` 下的各种 trait |
| **适配器（Adapter）** | 端口的具体实现——真正去读系统时钟、写文件、启动子进程的代码。在 `src/adapters/` 下 |
| **装配容器（DependencyGraph）** | 程序启动时把所有实现创建好、配好的地方，其它代码要用某个能力就向它要。在 `src/composition/dependency_graph.rs` |
| **关闭按钮行为** | 一项设置：点窗口关闭按钮时，是缩到系统托盘（`CloseToTray`）还是直接退出（`Exit`） |
| **CLI 输出** | N_m3u8DL-CLI 子进程打印的内容（进度、错误等），按块存储，不是塞进一个大文件 |

## 关键文件位置

- 程序入口：`src-tauri/src/lib.rs`（Tauri 启动 + 注册命令）
- 装配起点：`src-tauri/src/composition/app_bootstrap.rs` 和 `dependency_graph.rs`
- 架构规则的"活文档"：`src-tauri/tests/architecture_guard_static.rs`（规则以测试形式存在，改架构会先在这里失败）
- 产物盘点 port：`src-tauri/src/ports/artifact_inventory.rs`（ADR-0005 已实施）
- 产物定位策略：`src-tauri/src/application/artifact_location.rs`（ADR-0005 已实施，纯函数 `locate_artifact`）
- 前端状态：`src/lib/stores.js`（Svelte 的 store）
- 前端调用后端的封装：`src/lib/queue-store.js`、`settings-store.js`、`history-store.js`

## 运行时数据存在哪

`%APPDATA%\m3u8-queue-downloader\` 下：任务历史、CLI 输出、队列状态、设置。
