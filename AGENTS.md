# AGENTS

## 项目概览

- 根目录同时包含两个项目：
  - `cli/N_m3u8DL-CLI/`：旧 CLI（.NET Framework，已归拢进 `cli/`）
  - `m3u8-queue-downloader/`：当前桌面 GUI（Tauri + Svelte）
- 日常 GUI 改动、测试、打包脚本，优先看 `m3u8-queue-downloader/`。

## 目录约定

- GUI 前端与 Tauri 代码：`m3u8-queue-downloader/`
- GitHub Actions：`.github/workflows/`
- 本地打包产物目录：`../artifacts/`
  - 绝对路径：
    `D:\Downloads\N_m3u8DL-CLI_v3.0.2_with_ffmpeg_and_SimpleG\artifacts`

## 工具与索引

### 工具使用规则（强制，不得绕过）

本机为提升效率和准确度特地配置了三个工具，各有所长。以下规则按**问题类型**强制指定，**违反=错误，必须纠正后重做**。不要因为"顺手"就用默认的 grep 或全文件扫描替代——实测中它们都比指定工具慢且不准。

#### 决策表

| 你要做什么 | 必须用 | 禁止 | 不可用时 |
|---|---|---|---|
| **语义定位**："逻辑在哪儿"、某功能的实现位置、概念归属、不记得确切符号名 | `fast_context_search` | — | 无等价工具；失败须如实报告"已失去语义能力"，降级 Grep+Read 但结果可能不准，须告知用户 |
| **改动影响面 / 重构前调研**：改 X 会影响哪些地方、blast radius | `codegraph_explore` | — | 回退 `fast_context` + Grep+Read 手动推导，并告知用户已降级 |
| **精确调用关系**：谁调用生产函数 X、X 调用了谁 | `codegraph_callers` / `codegraph_callees` | — | 回退 Grep 找符号 + Read 读上下文手动推导 |
| **文本搜索**：找字符串、正则匹配、按扩展名过滤、核查文件是否存在 | `rg`（ripgrep） | `grep`/`Select-String` | rg 为本机必装工具，不可用视为环境故障，报错让用户修，不回退 |
| **读单个已知符号的源码** | `codegraph_node`（符号模式） | — | 回退 Read |

#### 场景重叠时的裁决（消除歧义）

实测发现三个工具能力有重叠，按问题**意图**裁决，不是按关键字：

| 问题意图 | 裁决 | 理由（实测依据） |
|---|---|---|
| "谁调用 `resolve_close_action`？"（精确调用关系） | **codegraph** | 唯一能直接给"X 有 N 个 caller 在哪些文件"的工具 |
| "`CloseToTray` 的逻辑/实现在哪儿？"（语义定位） | **fast context** | 一次返回 5 个最相关文件，含不含关键字的逻辑相关文件都能找到 |
| "我要改 `TaskStatus`，先了解影响面"（重构调研） | **codegraph_explore** | 给 blast radius（14 个 caller）+ 源码，fast context 只给文件名 |
| "找所有 `TODO` / 所有 `.svelte` 文件含 X"（纯文本） | **rg** | 字符串匹配是 rg 本职 |

#### 已知陷阱（实测踩过）

- **"谁调用 X" 不一定用 codegraph**：如果 X 是**测试函数 / main / 叶子入口**，`codegraph_callers` 会返回"No callers found"（正确，但答非所问）。此时改用 rg 找 X 的所有出现位置，或 fast context 定位其上下文。
- **codegraph 的杀手锏是 `explore`，不是 `callers`**：`explore` 一次性给"谁依赖它 + 源码"，是重构前调研的首选；`callers` 只给调用关系，适合精确单点查询。
- **codegraph 的"测试覆盖"信号不可信**：`explore` 输出里的 `⚠️ no covering tests found` 既可能准（如 `QueueManager` 所在文件确为 0 测试），也可能误报（如 `TaskRunner`/`HistoryStore`/`TaskStatus` 所在文件分别有 8/9/4 个测试，仍被报"无覆盖"）。根因是它按符号名匹配测试，而本项目测试是行为驱动命名（如 `running_task_remains_registered_until_process_exits` 测 `TaskRunner` 但函数名不含 `TaskRunner`）。**结论：判断测试覆盖必须用 `rg -c "#\[test\]" <文件>` 实数，不得采信 codegraph 的覆盖标注。**
- **fast context 不做调用图**：问"谁调用 X"时它只会返回相关文件，**答非所问**，别用它回答调用关系问题。
- **跨会话延续必须重新核实仓库状态**：新会话开始或长时间中断后继续工作时，第一步必须 `git status` + 扫 `docs/adr/`，确认工作区有无在途改动、有无新 ADR。不得沿用旧 context 的快照。实测踩坑：曾因不知道会话外产生了 ADR-0004 + 在途改动，整轮讨论基于过时状态。

### 架构评审与重构前必读（强制）

**违反=错误，必须纠正后重做。** 实测踩坑：曾基于过时快照把"已决策的设计"当成 friction 提议推翻，浪费整轮评审。

发起任何"架构评审 / deepening 分析 / 重构提议"之前，**按顺序完成**：

1. **读 `docs/adr/` 下全部 ADR**——这是已裁决的架构决策。如果提议与某条 ADR 冲突，必须在提议里明确指出"本提议推翻 ADR-XXXX，理由是..."，不得假装 ADR 不存在。
2. **跑 `git status` 确认工作区状态**——确认有没有在途改动（未提交的修改）。评审基于哪个基准（HEAD 还是工作区）必须在结论里写明。若工作区有在途改动，**先弄清是谁改的、改完没有**，不得在半成品工作区上发起新重构。
3. **判断"壳 vs 编排层"用标准，不用行数**——真壳的判据是：有没有自己的决策、事务边界、错误策略、权限收窄、事件语义或外部协议适配。纯转发=真壳；加了上述任一=合法编排层。文件小不等于壳。
4. **"刚做过这个方向"不是否决理由**——合并也可能过头。真正的否决理由是内聚性/耦合/不变量分析，不是"方向相反"。

特别针对 `queue_scheduling_orchestrator.rs`（中心调度 module）：**`ADR-0004` 已否决拆分**——理由是 QueueStart / QueueAdd-Retry / ChildExit 三场景共享同一调度执行流与状态机，强行拆分会导致网状跨模块互调。任何想拆它的提议，**必须先反驳 ADR-0004 的内聚性否决理由**（例如证明新场景导致 port 子集可完全分离、调度循环不再共享）。这不是永久禁令——若架构演化使 ADR-0004 的前提不再成立，拆分可能重新成立，但论证责任在提议者。*(注：这并不限制对该文件内部进行非破坏性的重构，例如在同一文件内提取无状态的 Helper 计算函数、私有助手结构，或在 ADR-0004 指引下通过引入 sub-ports 进行局部拆解与委托，以维持整体编排生命周期一致)*。

### 索引说明

- codegraph 索引库在外层 `../../.codegraph/`（覆盖整个工作区，含旧 CLI + GUI）。
- 在本子仓库任意目录跑 `codegraph sync` 都会更新同一个外层索引库。

### Windows 保留设备名陷阱

`nul`/`con`/`aux`/`prn`/`com1` 等是 Win32 保留设备名。`Get-ChildItem -Filter "nul"` 会**在每个目录下假阳性匹配**到 `NUL` 设备（报"存在"但磁盘上无真实文件），导致误以为有大量垃圾文件。核查文件是否存在时，用 `rg --files -g "<name>"`、`Test-Path`、或 `Get-ChildItem | Where-Object Name -eq "<name>"`（管道后过滤，非 `-Filter`）——这些都正确返回设备名的真实状态。


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

## Agent skills

### Issue tracker

GitHub Issues（通过 `gh` CLI），外部 PR 不作为 triage 入口。详见 `docs/agents/issue-tracker.md`。

### Triage labels

使用默认 triage 标签词表（`needs-triage` / `needs-info` / `ready-for-agent` / `ready-for-human` / `wontfix`）。详见 `docs/agents/triage-labels.md`。

### Domain docs

单一上下文布局（根目录 `CONTEXT.md` + `docs/adr/`）。详见 `docs/agents/domain.md`。

