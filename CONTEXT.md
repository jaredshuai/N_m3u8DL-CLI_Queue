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
| **任务状态** | 四种：`Waiting`（等待中）、`Downloading`（下载中）、`Completed`（完成）、`Failed`（失败）。**注意**：同一个状态在每一层有各自的类型，详见 `docs/adr/0003-*.md` |
| **队列（Queue）** | 一组排好序的任务，串行处理（同一时间只下载一个） |
| **活跃任务** | 还没结束的任务——也就是"等待中"或"下载中"的。代码里叫 `is_live_work`，用来判断队列是不是"还活着" |
| **历史（History）** | 已经到头的任务（完成或失败）留下的不可变记录 |
| **端口（Port）** | 后端内部的一种约定（interface）：某一层声明"我需要别人提供什么能力"，比如"我需要一个时钟""我需要能存任务的仓库"。代码里是 `src/ports/` 下的各种 trait |
| **适配器（Adapter）** | 端口的具体实现——真正去读系统时钟、写文件、启动子进程的代码。在 `src/adapters/` 下 |
| **装配容器（DependencyGraph）** | 程序启动时把所有实现创建好、配好的地方，其它代码要用某个能力就向它要。在 `src/composition/dependency_graph.rs` |
| **关闭按钮行为** | 一项设置：点窗口关闭按钮时，是缩到系统托盘（`CloseToTray`）还是直接退出（`Exit`） |
| **CLI 输出** | N_m3u8DL-CLI 子进程打印的内容（进度、错误等），按块存储，不是塞进一个大文件 |

## 关键文件位置

- 程序入口：`src-tauri/src/lib.rs`（Tauri 启动 + 注册命令）
- 装配起点：`src-tauri/src/composition/app_bootstrap.rs` 和 `dependency_graph.rs`
- 架构规则的"活文档"：`src-tauri/tests/architecture_guard_static.rs`（规则以测试形式存在，改架构会先在这里失败）
- 前端状态：`src/lib/stores.js`（Svelte 的 store）
- 前端调用后端的封装：`src/lib/queue-store.js`、`settings-store.js`、`history-store.js`

## 运行时数据存在哪

`%APPDATA%\m3u8-queue-downloader\` 下：任务历史、CLI 输出、队列状态、设置。
