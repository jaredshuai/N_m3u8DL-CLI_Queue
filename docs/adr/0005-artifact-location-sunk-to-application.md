# ADR-0005：产物定位策略下沉至 application 层，引入 ArtifactInventory port

- **状态**：已采纳（2026-06-21，经五轮独立 AI 审计 + 用户终审通过；2026-06-23 stage 4.4 follow-up 完成收尾）
- **决策依据**：候选 1（产物定位 / 完成事件构造泄漏穿过 adapter seam）的 grilling 设计树

## Stage 4.4 follow-up（2026-06-23 完成）

stage 4.4 加了 `TaskSnapshot.artifact_diagnostic` 字段 + `StoredArtifactDiagnostic` 持久化镜像，但当时**填充逻辑未接线**——`resolve_completed_artifact` 返回完整 `ArtifactResolution`，`handle_completed_child_exit` 只取 `Located` 的 path，丢弃了 `InventoryUnavailable` 的诊断。

**收尾实施**（commit 待填）把 diagnostic 真正贯通到 `TaskSnapshot`：

- 扩展 `QueueRepository::stage_task_completion` port signature 加 `artifact_diagnostic: Option<&ArtifactDiagnostic>` 参数
- `handle_completed_child_exit` 从 resolution 构造 diagnostic（仅 `InventoryUnavailable` → `Some`），穿过 `complete_child_exit` → `handle_completed_child_exit_history` → `handle_completed_task_history` → `stage_task_completion` 五层
- adapter `QueueManager::stage_task_completion` 在已有的 output_path 回填旁边加 diagnostic 回填
- 持久化路径已就绪（`StoredArtifactDiagnostic` 双向 From 在 stage 4.4 已加）
- 新测试 `task_completion_with_inventory_failure_persists_diagnostic` 验证 inventory 失败时 history 记录里 `output_path=None` 且 `artifact_diagnostic=Some(PermissionDenied, ...)`

设计选择：`NotFound`（目录存在但无匹配条目）**不**记录 diagnostic——这是"正常没找到"而非故障；只有 `InventoryUnavailable`（盘点本身失败）才记录。

## 问题在哪

下载子进程（N_m3u8DL-CLI）成功退出后，后端要在下载目录里找到产物文件（视频）的路径，存进任务历史。当前这条链路存在 seam 泄漏：

1. **产物定位策略驻留 adapter**：`find_output_file`（`adapters/task_runner.rs:629`，约 80 行）在 adapter 内同时做文件系统访问（`std::fs::read_dir`）和业务决策（扩展名白名单 `["mp4","mkv","ts","flv","mpg","mpeg"]`、save_name 前缀匹配、60 秒新鲜度窗口、mtime 降序排序）。业务策略不该在 adapter。
2. **adapter 反向构造 application 层领域事件**：`spawn_wait_task`（`task_runner.rs:325-351`）在 adapter 内自行构造 `TaskLifecycleEvent::Completed { output_path }`（application 层类型），经 mpsc channel 向上发。seam 两侧职责倒灌。
3. **adapter 内 mutate application snapshot**：`QueueManager::stage_task_completion`（`queue_manager.rs:128-159`）在 adapter 内把 `output_path` 回填到 application 的 `TaskSnapshot` 字段。
4. **`now` 隐式取系统时间**：`find_output_file` 直接调 `std::time::SystemTime::now()`（`task_runner.rs:672`），违反 application 不得直接碰系统资源的约束（当前能跑通只因它在 adapter 里；下沉后若不处理，application 策略函数就会碰系统时间）。
5. **错误语义被静默吞掉**：目录不存在 / 无权限 / IO 错误 / 单 entry 读失败，当前一律 `unwrap_or_default()` 填空字符串，下游无法区分"正常没找到"和"盘点出故障"。

本决策要把产物定位从 adapter 移到 application，重新画 application↔adapter 的 seam。

## 决定怎么做

### 1. 引入 `ArtifactInventory` port（application 声明，adapter 实现）

```rust
// ports/artifact_inventory.rs
trait ArtifactInventory: Send + Sync {
    fn snapshot(&self, dir: &ArtifactDir)
        -> Result<ArtifactDirectorySnapshot, ArtifactInventoryError>;
}
```

port **只传原始条目快照，不传过滤后的结果**。adapter 只报文件系统事实（read_dir + metadata + raw dirent kind），不做扩展名/前缀/新鲜度判断。理由：产物定位的 depth 在策略（~80 行业务规则），不在读目录（3 行 std::fs）；策略留 application 纯函数，port interface 保持小而深。

### 2. 产物定位策略是 application 的纯函数

```rust
// application/artifact_location.rs
fn locate_artifact(
    snapshot: &ArtifactDirectorySnapshot,
    request: &ArtifactLocateRequest,   // { save_name: Option<String> }
    policy: &ArtifactLocatePolicy,
    now: InventoryMoment,              // 调用时刻，从 Clock port 注入，用于新鲜度判断
) -> Option<ArtifactPath>
```

策略输入四类（snapshot 事实 / request 任务事实 / policy 规则 / now 时间事实）显式分离。`now` 复用已有 `Clock` port（`ports/clock.rs`，返回 `DateTime<Utc>`），不自己 `SystemTime::now()`。

**时间类型区分（2 号审计修正）**：`now`（调用时刻，wall clock，用于新鲜度窗口判断）和 `ObservedArtifactEntry.modified_at`（文件 mtime，文件系统元数据，用于排序）是**两个不同的时间概念**，不用同一个类型名：
- `now` 参数类型用 `InventoryMoment`（独立 newtype 包装 `DateTime<Utc>`，语义是"盘点发生的时刻"）
- entry 的 `modified_at` 类型用 `ArtifactModifiedAt`（独立 newtype 包装 `DateTime<Utc>`，语义是"文件最后修改时间"）
- 两者都是 `DateTime<Utc>` 的 newtype 包装，但语义不同，不混用类型名

### 3. `ArtifactLocatePolicy` 是 application policy 类型，不接 Settings

```rust
struct ArtifactLocatePolicy {
    allowed_extensions: Vec<ArtifactExtension>,  // normalized lowercase 不带点
    freshness_window: ArtifactFreshnessWindow,
    accepted_kinds: AcceptedArtifactKinds,       // { file: bool, symlink: bool }
}
impl ArtifactLocatePolicy {
    fn default_for_n_m3u8dl_cli() -> Self { /* mp4/mkv/ts/flv/mpg/mpeg, 60s, file+symlink=true */ }
}
```

`policy` 作为 `locate_artifact` 的显式输入参数（不是函数内隐式常量），让策略 module 的依赖完整、测试可参数化。当前用默认常量，不接 Settings——产物定位规则对用户太底层，没有用户反馈要调；将来要接 Settings 是增量改动（加 `policy_from_settings`，类型不变）。

### 4. 错误语义：修正版乙

port 的 `snapshot` 失败分三类：

| 情况 | 返回 | 理由 |
|---|---|---|
| 目录不存在 | `Ok(snapshot { presence: Missing, entries: vec![] })` | 子进程可能把货存别处，事实是"这里没有"，不是出错。但 `presence: Missing` 和空目录区分，不吞配置错误线索 |
| 无权限 / IO 异常 | `Err(ArtifactInventoryError::ReadDirectoryFailed { .. })` | 真故障，不吞 |
| 单 entry 读 metadata 失败 | snapshot 内跳过 + `skipped_entry_count` | 尽力而为盘点，某文件被并发删不该让整次失败 |

### 5. 事件 payload 只带原始事实，`ArtifactResolution` 是 application 内部类型

**事件流动时间线**（2 号审计要求明确）：

```
1. adapter spawn_wait_task 等子进程退出，拿到 exit_status
2. adapter 构造 TaskLifecycleEvent::Completed { id, download_dir, save_name }
   （只带原始事实，不做 find_output_file，不构造 ArtifactResolution）
3. adapter 经 mpsc channel 发事件
4. composition RuntimeFacade::handle_task_lifecycle_event 收到 Completed
5. RuntimeFacade 调 application handle_completed_child_exit(id, download_dir, save_name)
6. application 在 handle_completed_child_exit 内:
   a. snapshot = artifact_inventory.snapshot(&download_dir)?
   b. artifact = locate_artifact(&snapshot, &request{save_name}, &policy, clock.now())
   c. 按 artifact 分支投影到 TaskSnapshot(output_path + artifact_diagnostic)
7. 编排器继续 drive_child_exit_queue_and_handle_shutdown_countdown 等后续
```

`TaskLifecycleEvent::Completed` payload：

```rust
// adapter 构造的事件——只带原始事实，不带策略结果
enum TaskLifecycleEvent {
    Completed {
        id: String,
        download_dir: ArtifactDir,    // 子进程用的下载目录（原始事实）
        save_name: Option<String>,    // 子进程用的 save_name（原始事实）
        // 注：不再有 output_path，也不带 ArtifactResolution
    },
    Failed { id: String, error_message: String },
    ...
}
```

**关键**：`TaskLifecycleEvent::Completed` 当前由 adapter 构造（`task_runner.rs:329` spawn_wait_task、`:384` begin_wait_for_test）。如果把 `ArtifactResolution` 放进 payload，adapter 就必须执行 `locate_artifact` 策略来构造它——需要注入 `Clock` + `ArtifactInventory` + `ArtifactLocatePolicy`，策略又塞回 adapter，重构白做（此为 1 号审计否决的方案，见代价风险段）。

因此 `Completed` 只携带"子进程退出后的原始事实"（id + download_dir + save_name），`ArtifactResolution` 降为 **application 内部类型**，由 `handle_completed_child_exit` 收到事件后自己算：

```rust
// application 内部，不进事件 payload
enum ArtifactResolution {
    Located(ArtifactPath),
    NotFound,
    InventoryUnavailable(ArtifactInventoryError),
}
```

**连带：清理 domain 层死参数（2 号审计发现）**。`domain/queue.rs:197` `QueueTasks::stage_task_completion(id, _output_path: &str)` 的 `_output_path` 是死参数（`_` 前缀，函数体完全不用）；`QueueAggregate::stage_task_completion`（`:416-429`）接收 `output_path` 也只是透传给死参数。output_path 本就不该在 domain 层（ADR-0003 + 守护测试 `task_entity_does_not_contain_runtime_fields` 禁止 domain 含 output_path）。下沉后 domain 的 `stage_task_completion` 签名改为 `stage_task_completion(id: &str)`，删除死参数，同步改 `QueueAggregate` 透传链。

### 6. `TaskSnapshot` 加 `artifact_diagnostic`（层级二）；`TaskRuntimeState` / `StoredTask` 保持 `Option<String>` 不改

```rust
// application 层 TaskSnapshot——用 ArtifactPath
struct TaskSnapshot {
    output_path: Option<ArtifactPath>,              // 原 Option<String>（task_snapshot.rs:36），改 Option<ArtifactPath>
    artifact_diagnostic: Option<ArtifactDiagnostic>, // 新增
}
struct ArtifactDiagnostic { kind: ArtifactInventoryErrorKind, message: String }
```

诊断信息进持久化历史（层级二），不只是日志。理由：`output_path: None` 永久丢失"为什么是 None"的解释，日志可能被清理；`InventoryUnavailable` 解释了一个持久化空值，该跟历史一起保存。前端暂不展示 diagnostic（已有 outputPath 降级逻辑够用），将来要展示是增量改动。

**类型边界收窄（审计修正）**：`ArtifactPath` 只在 application 的 `TaskSnapshot` 层使用。下层保持裸 `Option<String>` 不改：

- `TaskRuntimeState`（`application/task_runtime_state.rs:11`）保持 `output_path: Option<String>`——守护测试 `task_runtime_state_contains_all_runtime_fields`（`architecture_guard_static.rs:2801`）精确断言 `pub(crate) output_path: Option<String>` 字面量，改类型会编译/断言失败。`TaskRuntimeState` 是运行时状态镜像，不该为产物概念升格。
- `StoredTask`（`adapters/task_record.rs:71`）保持 `output_path: Option<String>`——这是 ADR-0003 的 adapter 层镜像，serde 序列化的物理类型。

**`StoredTask` 需加 `artifact_diagnostic` 镜像字段**（审计修正，ADR 原稿漏）：

```rust
// adapters/task_record.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredArtifactDiagnostic {
    pub kind: String,        // ArtifactInventoryErrorKind 的序列化镜像
    pub message: String,
}

pub(crate) struct StoredTask {
    // ... 现有字段 ...
    pub output_path: Option<String>,                    // 不改
    #[serde(default)]
    pub artifact_diagnostic: Option<StoredArtifactDiagnostic>,  // 新增，default 兼容旧数据
    // ...
}
```

`#[serde(default)]` 保证旧历史 JSON（无此字段）反序列化为 `None`，不崩。`StoredArtifactDiagnostic` 是 `ArtifactDiagnostic` 的 adapter 层镜像（ADR-0003 标准模式），隔离 serde 属性。

### 7. `ArtifactPath` 语义：observed, non-canonical, 不解析 symlink target

- adapter 返回 `entry.path()`（文件系统观察事实），**不由 application 拼 `dir + name`**（path 构造责任归 adapter，locality 更好）
- **不 canonicalize**：canonicalize 会引入权限/链接断裂新失败模式，且下游（前端展示）不需要 canonical path。将来"打开产物"功能若需要，在消费 path 的 adapter operation 里按需做，失败回退 observed path
- **不解析 symlink target**：返回 symlink path 本身，不追到 target
- **`ArtifactDir` 必须 absolute non-canonical**：相对路径持久化进历史后工作目录变了会漂移；adapter resolve 阶段用 `current_dir().join()` 绝对化（非 `canonicalize`，保留 non-canonical）

### 8. symlink 不默认排除（保持当前行为）

策略默认 `accepted_kinds = { file: true, symlink: true }`，Directory/Other 排除。理由：本次重构主目标是把策略移到 application + 保留诊断事实，**不应顺手收紧 symlink 行为**。是否收紧交给 `ArtifactLocatePolicy.accepted_kinds` 配置，将来有用户数据再说。

### 9. 平台约束：只考虑 Windows

用户声明只在 Windows 上使用。匹配语义按 Windows 行为定：
- 扩展名/文件名匹配：**ASCII lowercase 归一化后比较**。行为变更（非纯保持）：exact match 当前用 `Path::exists()`（Windows 大小写不敏感），read_dir 分支当前用 `starts_with`/`ends_with`（字符串大小写敏感）；归一化后两分支统一为大小写不敏感，**扩大**了 read_dir 分支匹配范围。另外当前代码不检查 file kind（目录名以 `.mp4` 结尾也可能被返回），ADR 改为 Directory/Other 排除是**合理收紧**。
- exact match：**snapshot entries 内字符串相等**（非文件系统 `exists` 探测——这是单原语 `snapshot` 的必然结果，策略函数拿不到文件系统 exists 能力）
- mtime 排序：**mtime desc, name asc**（加 name tie-breaker，可预测，不依赖 read_dir 顺序）

## 为什么这么定

### 为什么用 port，不用纯函数

ADR-0001 强制 application 不得依赖具体实现/框架。`std::fs::read_dir` 是具体 IO，纯函数下沉会让 application 直接碰文件系统，守护测试 `application_layer_has_no_adapter_or_framework_dependencies` 会拦。

### 为什么 port 传原始条目，不传过滤后的结果

depth 在策略，不在读目录。若 port 传过滤后结果（`locate(dir, save_name, exts, freshness)`），interface 会随策略演化膨胀（明天加"按大小过滤"，port 签名得改），且测试 fake 得重新实现一遍策略，测的是 fake 的策略而非真实 adapter 的策略。port 传原始条目，策略留 application 纯函数，fake 与真实 adapter 共用同一套策略函数——这才是真 seam（两个 adapter 共用一个被测的策略）。

### 为什么不引入"CompletedButArtifactMissing"中间态

下载确实成功了（子进程退出码 0），产物路径缺失只是元数据缺失，不构成新任务状态。`Completed` 就是 Completed，产物有无由 application 内部的 `ArtifactResolution` 表达（不进事件 payload，见决策 5），投影到 `TaskSnapshot.output_path: Option<ArtifactPath>` + `artifact_diagnostic: Option<ArtifactDiagnostic>`。

### 为什么 `TaskSnapshot.output_path` 从 `Option<String>` 改 `Option<ArtifactPath>` 并新增 `artifact_diagnostic`

当前 adapter 的 `find_output_file` 找不到产物时返回 None，`spawn_wait_task` 用 `unwrap_or_default()` 把 None 压成空字符串塞进事件 payload，下游无法区分"没找到"和"出错了"。决策 5 已把产物定位上移到 application，事件 payload 不再含 output_path（只带原始事实 id/download_dir/save_name）。application 的 `handle_completed_child_exit` 算出 `ArtifactResolution` 后投影到 `TaskSnapshot`：`output_path: Option<ArtifactPath>` 让"有/无"在类型层面可区分；新增 `artifact_diagnostic: Option<ArtifactDiagnostic>` 让"为什么无"的解释跟历史一起保存，不只靠可能被清理的日志。注意 `TaskLifecycleEvent::Completed` 本身不含 output_path/artifact_diagnostic——这些是 `TaskSnapshot` 的字段，不是事件 payload。

## 代价和风险

- **代价**：新增 port + 一组类型（`ArtifactInventory`/`ArtifactDirectorySnapshot`/`ObservedArtifactEntry`/`ArtifactPath`/`ArtifactDir`/`ArtifactModifiedAt`/`InventoryMoment`/`ArtifactEntryKind`/`ArtifactInventoryError`/`ArtifactInventoryErrorKind`/`ArtifactDirectoryPresence`/`ArtifactResolution`/`ArtifactDiagnostic`/`ArtifactLocatePolicy`/`ArtifactLocateRequest`/`AcceptedArtifactKinds`/`ArtifactExtension`/`ArtifactFreshnessWindow` + adapter 镜像 `StoredArtifactDiagnostic`），共约 18 个。类型数量多，但大部分是职责清晰的 newtype（每个一两行定义），真正有业务逻辑的只有 `locate_artifact` 函数和 `ArtifactInventory` adapter 实现，符合 ADR-0003 层间类型镜像模式。
- **代价**：`QueueSchedulingPorts` 从 9 port 扩到 11 port（加 `Clock` + `ArtifactInventory`），牵动构造器签名 + `DependencyGraph` 工厂 + 所有测试构造点（详见风险段）。不违反 ADR-0004（反对拆文件，不限制 port 数量）。
- **代价**：`application/mod.rs` 需按 ADR-0002 加 `ArtifactInventory` 的 re-export（`pub(crate) use crate::ports::artifact_inventory::ArtifactInventory;`），否则调用方写 `ports::ArtifactInventory` 而非 `application::ArtifactInventory`，语义方向不对。
- **代价**：`TaskSnapshot` 加字段 + `StoredTask` 加镜像字段，`StoredTask.artifact_diagnostic` 需 `#[serde(default)]` 兼容旧历史 JSON（旧数据无此字段时反序列化为 None，不崩）。
- **代价**：`TaskLifecycleEvent::Completed` payload 改类型（从 `output_path: String` 改为 `id + download_dir + save_name` 原始事实），牵动所有构造 Completed 的地方（adapter `spawn_wait_task:329` / `begin_wait_for_test:384`）+ 消费方（`RuntimeFacade::handle_task_lifecycle_event:41` 的 match 分支 + `handle_completed_child_exit:497` 签名 + `task_runner.rs:917` 测试辅助的 match 消费点——该处用 `..` 忽略字段，payload 改字段后仍可编译，低影响但应列入影响面）。
- **风险**：exact match 从文件系统 `exists` 改为字符串相等，在 Windows 大小写不敏感文件系统上靠 ASCII lowercase 归一化保持行为；但若 N_m3u8DL-CLI 输出非 ASCII 扩展名（极罕见），归一化可能不够。需在策略测试覆盖。
- **风险**：symlink 不排除是行为保持，但 Windows 上 symlink 需管理员权限创建，实际极少见。若将来发现 symlink 误中产物，收紧 `accepted_kinds.symlink = false` 即可，不需改架构。
- **风险（审计修正）**：`TaskRuntimeState` / `StoredTask` 保持 `Option<String>` 不改，`ArtifactPath` 只在 `TaskSnapshot` 层用——这意味着 `TaskRuntimeState` → `TaskSnapshot` 转换时要做 `Option<String>` → `Option<ArtifactPath>` 映射。映射点是新的转换边界，需测试覆盖。
- **风险（审计修正）**：`handle_completed_child_exit` 收到 `Completed{id, download_dir, save_name}` 后要做 snapshot + locate_artifact 两步。这两步当前在 adapter 闭包里是同步完成的，移到 application 后变成 async 调用链，需确认 `QueueSchedulingPorts` 已持有 `ArtifactInventory` port（否则要扩 port bundle，牵动 ADR-0004 的 9-port 编排器）。
- **风险（4 号审计修正，实施代价被低估）**：经核实，`QueueSchedulingPorts`（`queue_scheduling_orchestrator.rs:37-47`）当前 9 个 port **不含** `Clock` 和 `ArtifactInventory`。实施时**必须扩展到 11 个 port**：加 `clock: &'a dyn Clock`（用于 `locate_artifact` 的 `now` 参数）和 `artifact_inventory: &'a dyn ArtifactInventory`（用于 `snapshot`）。这牵动：(1) `QueueSchedulingPorts::new` 构造器签名（`queue_scheduling_orchestrator.rs:50`）；(2) `DependencyGraph::queue_scheduling_orchestrator()` 工厂方法（`dependency_graph.rs:80-95`）需传入这两个依赖；(3) 所有测试中直接构造 `QueueSchedulingPorts` 的地方（`lib.rs:658,889,959,1016` 等集成测试）。ADR-0004 说"代价：依赖总数较多（9 个）"——扩展到 11 个应在 ADR-0005 代价段明确提及，避免后人困惑。这不违反 ADR-0004（ADR-0004 反对的是物理拆分文件，不是 port 数量）。
- **风险（审计修正）**：事件处理 latency。`snapshot()` 是 IO（read_dir），放进 `handle_completed_child_exit` 的事件处理路径会增加 latency。当前走 tokio::spawn 不阻塞 Tauri 主线程，但 ADR-0005 原稿未提及。实施时确认 `handle_task_lifecycle_event` 仍异步且不在主线程阻塞。
- **风险（审计修正）**：守护测试 `task_runtime_state_contains_all_runtime_fields`（`architecture_guard_static.rs:2801`）精确断言 `pub(crate) output_path: Option<String>` 字面量。本决策保持 `TaskRuntimeState.output_path: Option<String>` 不改，此断言**不受影响**。但 `domain/queue.rs` 删除 `_output_path` 死参数后，需确认无其他守护测试断言 `stage_task_completion` 的签名含 output_path（实施时核实）。
- **风险（审计修正）**：domain/queue.rs 的 `stage_task_completion` 死参数清理（决策 5 连带）牵动 `QueueTasks::stage_task_completion`、`QueueAggregate::stage_task_completion`、`application_queue_completion_staging_outcome`、`queue_manager.rs:128` 调用链。改动面不小但都是机械删除参数，测试保护。
- **风险（4 号审计修正）**：`TaskRuntimeState::mark_completed(output_path: &str)`（`task_runtime_state.rs:79`）当前由 `queue_manager.rs:149` 调用，output_path 来自事件 payload 的字符串。下沉后，output_path 来源变为 application 内部的 `ArtifactResolution::Located(path)` 提取。`TaskRuntimeState` 保持 `Option<String>` 不变，`mark_completed` 签名不变，但**调用时机和调用方变化**：从 adapter `QueueManager` 拿事件 payload 字符串，变为 orchestrator 从 `ArtifactResolution::Located(path)` 提取 `path.into_string()` 再传入。`ArtifactPath → String` 的拆包点是新边界，需测试覆盖。
- **风险（3 号审计修正，影响面被低估）**：除 ADR 已列的 spawn_wait_task / begin_wait_for_test / RuntimeFacade / handle_completed_child_exit，还会牵动：
  - `QueueRepository::stage_task_completion` port interface（`ports/queue_repository.rs:62`，含 output_path 参数）——**分两步判断**：(a) domain 死参数（`_output_path`）必删，牵动 `QueueTasks`/`QueueAggregate`/outcome/queue_manager 调用链；(b) port 签名是否删 output_path，取决于决策5+6 把 `mark_completed`（queue_manager.rs:149）和 snapshot 回填（:158-159）上移到 orchestrator 后，adapter 是否还需该参数。当前 adapter 仍用 output_path 做 `mark_completed` + 回填，所以 port 签名删除是**决策5+6 完全落地后**的后续判断，不是 domain 删参的必然牵动。实施时先删 domain 死参，port 签名删除作为独立决策评估。
  - `TerminalHistoryPorts::stage_task_completion`（`terminal_history_orchestrator.rs:82`）——同链路
  - query model / DTO 的 output_path 投影（`query_models.rs:35,52` + `frontend_dto.rs:40,57,180`）——若 output_path 语义变化需同步
  - `lib.rs` 里直接调 `handle_completed_child_exit` 的集成测试（rg 核实：lib.rs:697,935,1000,1059，均传字符串 `"D:/Videos/test.mp4"`；行号可能随代码漂移，实施时以实际为准）——签名变化需同步改测试
- **风险（3 号审计修正，行为变更表述）**：决策 9 说"保 Windows 大小写不敏感行为"，但这不完全准确。当前 exact match 用 `Path::exists()`（Windows 大小写不敏感），但 `read_dir` 分支的 `starts_with`/`ends_with` 是**字符串大小写敏感**（`task_runner.rs:651-654,678-680`）。ADR 的 ASCII lowercase 归一化会**扩大** read_dir 分支的匹配范围（从大小写敏感变不敏感），不是纯保持。另外当前代码不检查 file kind，目录名以 `.mp4` 结尾也可能被返回；ADR 改为 Directory/Other 排除是**合理收紧**，但应表述为行为变更而非纯保持。
- **风险（3 号审计修正，守护测试影响面）**：除 `task_runtime_state_contains_all_runtime_fields`，还会触发：
  - `event_handlers` 不应直接处理 `TaskLifecycleEvent::Completed`（`architecture_guard_static.rs:1066-1079`）——若事件处理路径变化需核实
  - `terminal_history` 不应直接调低层 staging 方法（`:1504-1515`），TerminalHistoryPorts 低层 helper 应保持私有（`:1575-1587`）——死参数清理时需核实不破坏
  - 现有回归测试断言完成历史持久化 output_path（`lib.rs:1071-1085`），需改成覆盖 Located/NotFound/InventoryUnavailable 三态
- **否决过的方案**：
  - 纯函数下沉（application 直接 std::fs）——违反 ADR-0001
  - port 传过滤后结果——interface 随策略膨胀，fake 要重写策略
  - port 双原语 `stat` + `list_entries`（保精确匹配 O(1)）——download_dir 是用户视频目录可能积累大量文件，但桌面语境 O(N) IO 可接受，单原语 `snapshot` 更干净
  - symlink 默认排除——重构不该顺手收紧策略，且 Windows 上 symlink 极罕见，收紧收益小
  - `ArtifactResolution` 只写日志不进 TaskSnapshot——`output_path: None` 会永久丢失解释，日志可能被清理
  - canonicalize ArtifactPath——引入新失败模式，下游不需要 canonical path
  - **`TaskLifecycleEvent::Completed` payload 带 `ArtifactResolution`（审计否决）**——adapter 是事件构造方，带 `ArtifactResolution` 会逼 adapter 执行 `locate_artifact` 策略，需注入 Clock + ArtifactInventory + Policy，策略又塞回 adapter，重构白做。改为 payload 只带原始事实，`ArtifactResolution` 降为 application 内部类型。
  - **`TaskRuntimeState.output_path` 改 `Option<ArtifactPath>`（审计否决）**——守护测试 `task_runtime_state_contains_all_runtime_fields` 精确断言 `pub(crate) output_path: Option<String>` 字面量，改类型会断言失败。`TaskRuntimeState` 保持 `Option<String>`，只在 `TaskSnapshot` 层用 `ArtifactPath`，转换时映射。
