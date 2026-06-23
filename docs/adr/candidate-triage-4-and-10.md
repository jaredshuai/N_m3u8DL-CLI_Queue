# 候选清单收尾：候选 4 与候选 10 裁决

- **状态**：裁决已采纳（2026-06-23，6 家 AI 一致裁决）
- **决策依据**：ADR-0001～0008 完成后剩余两个悬而未决候选的正式处置

## 背景

会话最初由一份架构评审报告（5 轮 AI 审计版）提出约 10 个候选。多数已实施或被早期否决。完成 ADR-0006/0007/0008 + 候选 7 + ADR-0005 stage 4.4 收尾后，只剩两个候选悬而未决。本文件给出正式裁决，避免未来误读为"还没做"。

## 候选 4：适配器持有 TaskSnapshot 应改 Port 返回

### 原文

> `TaskRunner`（adapter）内部构造 `TaskSnapshot`（application 层类型）。port 应该返回原始数据，由 application 构造 snapshot。

### 裁决：**降级为已消化**

### 一句话理由

ADR-0005 已将 `TaskLifecycleEvent::Completed` 的 payload 改为只带原始事实（`id` + `download_dir` + `save_name`），`TaskRunner` 不再构造 `TaskSnapshot` 或 `ArtifactResolution`；剩余的 `ArtifactDir` newtype 和 `TaskLifecycleEvent` 是 port 消息类型，不构成独立候选。

### 证据（6 家 AI 交叉核实）

- `task_runner.rs:335` 构造的是 `TaskLifecycleEvent::Completed { id, download_dir: ArtifactDir::new(s), save_name }`
- 全文件 grep `TaskSnapshot` 在 `task_runner.rs` 内零命中
- `ArtifactResolution` 计算已下沉到 `queue_scheduling_orchestrator.rs::resolve_completed_artifact`
- `ArtifactDir` 只是 `String` 的 newtype（`new(path: String)` 裸赋值），不含业务逻辑
- adapter 构造事件 payload 是六边形架构的有意设计（adapter 观察事实、application 处理决策）

### 触发条件（什么情况下应重新打开）

如果后续发现 adapter 再次开始构造/回填 application 层聚合类型（如 `TaskSnapshot`、`ArtifactResolution` 或带策略结果的 Completed payload），则重新打开。

## 候选 10：FileSystemArtifactInventory 拆分

### 原文

> `FilesystemArtifactInventory` adapter（257 行）做了 spawn_blocking + read_dir + metadata 分类 + 错误归类。可以按"读/分类/错误处理"三步拆。

### 裁决：**证伪移除**

### 一句话理由

`adapters/filesystem_artifact_inventory.rs` 已经按"读/分类/错误"自然拆成 `snapshot_sync`、`classify_kind`、`classify_io_error` 三个独立函数；257 行中约 80 行是测试，实际逻辑约 170 行且职责单一，原描述的问题已不成立。

### 证据（6 家 AI 交叉核实）

| 原文要求的步骤 | 当前实现 | 位置 |
|---|---|---|
| 读目录 | `snapshot_sync`（含 `fs::read_dir` + entries 遍历） | `:53-138` |
| 类型分类 | `classify_kind`（独立纯函数） | `:145-163` |
| 错误归类 | `classify_io_error`（独立纯函数） | `:165-176` |

- 候选描述的"三步拆"已全部存在
- 4 层嵌套 match（`dir_entry → metadata → modified_at → duration_since`）是 Rust 处理 `Result<Option<Result<_>>>` 的天然形态，每层有独立 fallback（read_dir 失败 → Err 返回；dir_entry/metadata 失败 → skip+continue；modified_at 失败 → 视情况 skip），提取成管道反而损失每层独立 `skipped += 1` 的精确性
- ADR-0007 评审时 6 家 AI 无人指出内聚问题

### 触发条件（什么情况下应重新打开）

如果该 adapter 新增第二种存储后端、或 `snapshot_sync` 内部出现可复用的跨 adapter 逻辑，使得"读/分类/错误"需要变成可共享的模块/类型，再考虑进一步拆分。如果只是想消减 `snapshot_sync` 的 4 层嵌套 match，应作为**新候选**（措辞为"消减嵌套"而非"三步拆"）独立记录。

## 候选清单最终状态

| 候选 | 状态 | 处置 ADR |
|---|---|---|
| 1：产物定位 seam 泄漏 | ✅ 已实施 | ADR-0005 |
| 2：QueueRepository 窄 trait 无调用方 | ✅ 已实施 | ADR-0006 |
| 3：Guard test 硬编码类名 | ✅ 已实施 | ADR-0007 |
| 4：adapter 持有 TaskSnapshot | 🔶 **降级为已消化** | 本文件 |
| 5：Orchestrator 内部变浅 | ✅ 已实施（候选 6 受限版） | ADR-0008 |
| 6：场景尾部副作用归并 | ✅ 已实施 | ADR-0008 |
| 7：schedule_next wrapper 链 | ✅ 已实施（最小版） | ADR-0008 Future Work |
| 8-9：（合并入上述） | — | — |
| 10：FilesystemArtifactInventory 拆分 | ❌ **证伪移除** | 本文件 |

清单已无悬而未决候选。

## 评审轨迹

- 6 家 AI（Claude / AtomCode / Kimi / Codex / 场外 1 / 场外 2）独立核查代码后给出一致裁决
- 候选 4：6/6 投"降级为已消化"
- 候选 10：6/6 投"证伪移除"