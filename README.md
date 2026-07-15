# m3u8 队列下载器

一个 Windows 桌面应用，给 [N_m3u8DL-CLI](https://github.com/nilaoda/N_m3u8DL-CLI) 套了个图形界面，支持排队下载、失败自动重试、下载历史。

真正的下载由 N_m3u8DL-CLI（命令行工具）完成，本仓库提供的是它的**队列管理界面**（Tauri + Svelte）。

## 下载

前往 [GitHub Releases 最新版本](https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest) 下载：

从 0.2.2 起，Release 使用以下版本化资产名：

- **安装版**：`m3u8-queue-downloader_<版本>_x64-setup.exe`
- **便携版**：`m3u8-queue-downloader_<版本>_portable_x64.zip`，解压后直接运行

两个版本都已内置 N_m3u8DL-CLI 和 ffmpeg，仅支持 Windows x64。

兼容提示：若 Latest 仍为 0.2.1，其便携包资产名为 `portable.zip`；安装包请按该 Release 页面列出的实际资产选择。

## 仓库结构

```
/
├── m3u8-queue-downloader/   ← 桌面 GUI（当前主力开发）
├── cli/                     ← 内置的 N_m3u8DL-CLI 源码（fork 自 nilaoda，冻结状态）
├── docs/                    ← 架构决策记录（ADR）+ agent 配置
├── .github/workflows/       ← CI / 打包
├── AGENTS.md                ← AI 协作约定（工具规则、打包流程等）
└── CONTEXT.md               ← 项目上下文与术语表
```

- 日常开发、打包都在 `m3u8-queue-downloader/` 下
- `cli/` 是作为外部子进程调用的下载内核，源码来自上游，不在日常维护范围

## 功能

- 批量添加 m3u8 链接，串行队列下载
- 下载失败自动重试
- 关闭窗口时最小化到系统托盘（可配置）
- 下载历史归档，支持查看 CLI 输出
- 自定义请求头、保存名、下载目录

## 开发

```bash
cd m3u8-queue-downloader
npm install
npm run tauri dev
```

打包相关说明见 [`AGENTS.md`](./AGENTS.md) 的「打包规则」一节——本机推荐走 GitHub Actions，不优先本地打包。

## 技术栈

- 后端：Rust（Tauri），六边形分层架构，详见 [`docs/adr/`](./docs/adr/)
- 前端：Svelte + Vite
- 下载内核：N_m3u8DL-CLI v3.0.2（.NET，外部子进程）+ ffmpeg

## 许可证

内置的 N_m3u8DL-CLI 遵循其自身的 MIT 许可证（见 `cli/LICENSE`）。
GUI 部分见 `m3u8-queue-downloader/` 内的许可声明。
