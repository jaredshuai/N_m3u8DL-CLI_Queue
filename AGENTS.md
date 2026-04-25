# AGENTS

## 项目概览

- 根目录同时包含两个项目：
  - `N_m3u8DL-CLI/`：旧 CLI（.NET Framework）
  - `m3u8-queue-downloader/`：当前桌面 GUI（Tauri + Svelte）
- 日常 GUI 改动、测试、打包脚本，优先看 `m3u8-queue-downloader/`。

## 目录约定

- GUI 前端与 Tauri 代码：`m3u8-queue-downloader/`
- GitHub Actions：`.github/workflows/`
- 本地打包产物目录：`../artifacts/`
  - 绝对路径：
    `D:\Downloads\N_m3u8DL-CLI_v3.0.2_with_ffmpeg_and_SimpleG\artifacts`

## 打包规则

- 这台机器上本地 Tauri/Vite 打包经常遇到 `EPERM`/ACL 问题。
- 默认不要优先走本地打包。
- **首选 GitHub Actions `Package GUI` workflow**。

## 推荐打包流程

在 `m3u8-queue-downloader/` 目录执行：

```powershell
node scripts/prepare-release.mjs package-sync --ref <远端分支名>
```

常用变体：

```powershell
npm run package:sync:master
node scripts/prepare-release.mjs package-sync --ref <远端分支名> --skip-tests
node scripts/prepare-release.mjs package-sync --run-id <已成功的_actions_run_id>
```

不要使用 `npm run package:sync -- --ref master`；当前 npm 版本会把 `--ref` 当作 npm 自身配置吞掉。

说明：

- `package:sync` 会触发 GitHub Actions `Package GUI`。
- 构建完成后，会把产物自动下载回根目录外层的 `artifacts/`。
- 打包的是 **GitHub 上已经存在的分支**，不是本地未推送改动。

## 当前约定产物

同步回本地 `artifacts/` 后，应看到：

- 安装包：
  `artifacts/m3u8-queue-downloader_0.1.0_x64-setup.exe`
- portable 文件夹：
  `artifacts/m3u8-queue-downloader-portable/`

portable 目录当前应至少包含：

- `m3u8-queue-downloader.exe`
- `resources/ffmpeg.exe`
- `resources/N_m3u8DL-CLI_v3.0.2.exe`

## GitHub Actions 约定

- `Package GUI`
  - 用于测试包/日常包
  - 产出 installer + portable 目录
  - 本地 `package:sync` 默认使用它
- `Release`
  - 用于 draft/prerelease/release 发布
  - 不作为日常测试打包首选

## 运行时数据排查

如果 GUI 出现历史数据导致的问题，运行时数据默认在：

- `%APPDATA%\\m3u8-queue-downloader`

必要时可只清理任务数据，保留设置：

- 删除：
  - `history/`
  - `cli-output/`
  - `queue_state.json`
- 保留：
  - `settings.json`

## 已确认的行为修复

- 托盘菜单“退出程序”应始终真正退出进程，不应受 `CloseToTray` 设置影响。
- 历史 CLI 输出已从单文件全量读取改为 chunked 存储读取。

