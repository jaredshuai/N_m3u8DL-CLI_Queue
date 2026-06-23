# ADR-0007：架构守护测试硬编码名称治理——意图注释 + 实现级 const 提取

- **状态**：已采纳（2026-06-23，经六轮 AI 讨论后用户裁定 C1）
- **决策依据**：候选 3（架构守护测试硬编码类名过脆）的两阶段讨论，最终选择 C1 方案

## 问题在哪

`architecture_guard_static.rs`（~3429 行）通过 `include_str!` 在编译时扫描源码，用 `assert!(source.contains("pattern"))` 验证架构约束。其中约 531 个 `.contains()` 断言里有约 120 个硬编码了具体的类型名、函数名和构造模式。

这些硬编码名称在两个方向上产生摩擦：

1. **碎时无方向感**：改名触发 guard 失败时，维护者需要推断"这个 assert 在守护什么语义"才能正确更新。当前断言在源文件中散落，没有统一的意图标注。
2. **实现级重命名碎片**：约 40 个断言检查的是内部私有方法名（如 `handle_task_failure_transition_error`、`pause_after_failure_persistence_error`），这些函数名的改动不改变架构 seam，但 guard 仍会碎裂。

## 决定怎么做

### 1. 架构级名称保持原位硬编码（约 80 个）

以下类别的名称保持 `source.contains("Name")` 形式不变：

| 类别 | 例子 |
|---|---|
| Ports 结构体 | `QueueSchedulingPorts`、`QueueMutationPorts`、`ExitPorts` 等 |
| Facade 结构体 | `DiagnosticsFacade`、`QueueCommandFacade`、`RuntimeFacade` 等 |
| Outcome/Request 类型 | `QueueTaskCompletionStagingOutcome`、`TaskProcessStartRequest` 等 |
| DTO 类型 | `QueueStateDto`、`TaskDto` 等 |
| Port trait 名称 | `QueueRepository`、`HistoryRepository`、`TaskProcessRunner` 等 |
| 重要适配器类型 | `TauriTaskProcessRunner`、`QueueManager`、`HistoryStore` 等 |
| 其他架构性类型 | `ProcessRunnerShutdownStatus`、`QueueRunStatus`、`RetryPolicy` 等 |

理由：这些名字的改名意味着架构结构的变更，guard 理应在改名时碎裂，强制人工重新确认拓扑。

### 2. 实现级内部方法名提取为同文件 const（约 40 个）

以下类别的名称提取为 `architecture_guard_static.rs` 文件顶部的 `const` 定义：

| 类别 | 例子 |
|---|---|
| 私有 helper 函数名 | `handle_queue_add`、`handle_task_removal`、`handle_tasks_reorder` |
| 内部编排方法名 | `create_task_process_runner`、`create_process_runner` |
| 持久化实现细节 | `record_completed_task_to_history`、`record_terminal_failure_task_to_history` |
| 内部 error handler | `handle_task_failure_transition_error` |
| 设置类内部方法 | `update_settings_and_handle_auto_action_change` |
| 其他实现级细节 | `state.task_runner`、`task_process_runner_factory` |

理由：这些名字属于内部实现细节，改名不改变架构 seam。提取为同文件 const 后，如果同一名字在多处出现，只需更新一处 const 定义值。const 定义在同一个文件中（非跨文件模块），不引入间接层。

**提取规则**：
- const 定义在 `architecture_guard_static.rs` 的 `assert_sources_do_not_contain` 函数附近
- 命名格式：全大写蛇形，加简短注释说明用途
- 仅在断言通过 `const NAME` 引用（如 `source.contains(MY_CONST)`）
- 不创建跨文件拓扑模块

### 3. 全部约 120 处加意图注释

架构级和实现级均添加注释前缀：

| 前缀 | 含义 | 例子 |
|---|---|---|
| `// 布线：` | 验证组件 A 被正确接线到组件 B | `// 布线：dependency_graph 必须实例化调度器端口` |
| `// 负向：` | 验证某组件**不**出现在不应出现的层 | `// 负向：facade 不得直接实例化端口` |
| `// 结果类型：` | 验证方法返回显式 Outcome 类型 | `// 结果类型：stage_completion 返回显式 Outcome` |

### 4. 不做的事情

- ❌ 不创建跨文件的拓扑声明模块（ADR-0007 已否决方案 A）
- ❌ 不改动 410 个层边界规则断言（`"crate::adapters"`、`"tauri::"` 等）
- ❌ 不删除任何断言（保护力不变）
- ❌ 不改变 guard 的逻辑行为

## 净效果

| 维度 | 改动前 | 改动后 |
|---|---|---|
| const 定义 | 0 行 | ~80 行（40 个 × 约 2 行每个） |
| 意图注释 | 0 行 | ~120 行 |
| 架构级改名保护 | ✅ 会碎 | ✅ 照碎 |
| 实现级改名摩擦 | 40 处碎 | 同文件 const，碎不减少但编辑集中 |
| 碎时方向感 | 无 | 每处有 `// 布线：` / `// 负向：` / `// 结果类型：` 指引 |

## 备选方案（已否决）

### A：集中拓扑模块

全票反对。引入跨文件间接层不解决本质问题。

### B：保持现状

6 票中 0 票纯 B。无人认为现状完全不可接受，但都认为加注释有净收益。

### C0：只加注释

6 票中 3 票选 C0。净正收益但零摩擦改善。

### C1（被选方案）

6 票中 3 票选 C1。Claude、AtomCode、Codex 一致认为 C1 的平衡最佳。