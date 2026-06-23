# ADR-0008：调度场景尾部副作用归并（受限版候选 6）

- **状态**：已采纳（2026-06-23，经 6 家 AI 两轮评审后用户裁定）
- **决策依据**：候选 5（inline mark 别名）两轮 6-AI 讨论后方向调整为候选 6，再经 ADR 草稿 6-AI 评审收敛

## 问题在哪

`queue_scheduling_orchestrator.rs`（988 行）中存在一类"场景尾部副作用"重复：

### 类型 A：`schedule_next + queue_state_changed` 两步模式（重复 3 处）

```rust
// complete_queue_add_scheduling（line 770）
self.schedule_next_if_requested(add_outcome.into()).await?;
self.mark_queue_add_scheduling_completed();          // → self.events.queue_state_changed()

// schedule_next_for_add_outcome_and_mark_history（line 781）
self.schedule_next_if_requested(add_outcome.into()).await?;
self.mark_queue_retry_history_scheduling_completed();

// schedule_next_for_existing_task_retry（line 904）
self.schedule_next_if_requested(ScheduleNextRequest::Requested).await?;
self.mark_queue_retry_existing_scheduling_completed();
```

三个场景（QueueAdd / QueueRetryFromHistory / QueueRetryExisting）尾部都做了"调度下一个 + 通知队列状态变化"两步，但用三个不同的 `mark_*` 别名包装 `queue_state_changed`。

### 类型 B：单步 mark 别名（散落 ~9 处，不在本 ADR 范围）

9 个 `mark_xxx(&self) { self.events.queue_state_changed(); }` 纯转发。第二轮讨论结论：保留作为 988 行状态机的"读者路标"。

## 上一轮讨论结论（6 AI 投票汇总）

- **议题 1（裸 inline 9 个 mark）**：2 票 inline / 4 票反对。
- **议题 2（做候选 6）**：5 票赞成 / 1 票反对。
- **议题 3（候选 6 是否需 ADR）**：4 票需 ADR / 2 票不需（基于"严格私有"前提）。

各家一致的**三个硬约束**：
1. 限定为同文件私有 enum/helper
2. 只做"场景结果 → 事件序列"映射，不拥有调度决策
3. 不抽新文件、不引入新 port、不做成 sub-orchestrator

## ADR 草稿评审结论（6 AI 第二轮）

| 评审点 | 6 家结论 |
|---|---|
| enum 必须改名 | 6/6 一致 |
| `outcome` 参数未被消费（YAGNI / 编译警告） | 6/6 一致必改 |
| ChildExit 边界判断正确 | 6/6 一致 |
| 无遗漏同构模式（codegraph_callers 确认 3 caller） | 6/6 一致 |

### Codex 抓到的草稿事实错误（已修正）

旧版草稿："QueueStart 尾部无事件副作用"——**错误**。代码核查显示：
- `handle_queue_start:416-420` 可能 emit `shutdown_countdown_cancelled`
- `pause_run_queue_start:463` 和 `start_run_and_schedule_next_internal_queue_start:486` 也 emit `queue_state_changed`

正确表述：QueueStart 有事件副作用，但**不是 `schedule_next_if_requested + queue_state_changed` 这种同构尾部**。

## 决定怎么做

### 1. 引入私有枚举 `QueueMutationScenario`（3 变体）

```rust
/// Identifies which queue mutation triggered a `schedule_next + queue_state_changed` tail.
/// Carries scenario identity for future diagnostics/metrics hooks (YAGNI guarded by immediate consumption).
enum QueueMutationScenario {
    QueueAdd,
    RetryFromHistory,
    RetryExisting,
}

impl QueueMutationScenario {
    /// Emit queue-state-changed event for this scenario.
    /// Currently identical across scenarios; the discriminant is preserved as
    /// a future metrics/telemetry hook point (consumed here, not dead weight).
    fn emit_queue_changed(self, events: &dyn FrontendEventPublisher) {
        match self {
            QueueMutationScenario::QueueAdd
            | QueueMutationScenario::RetryFromHistory
            | QueueMutationScenario::RetryExisting => events.queue_state_changed(),
        }
    }
}
```

`match self` 立即消费 discriminant，避免 unused 警告；同时为未来 metrics 留扩展点（Kimi 方案）。

只覆盖**类型 A**（3 个 `schedule_next + queue_state_changed` 场景），不强行覆盖散落的类型 B 别名。

### 2. 引入私有 helper `complete_queue_mutation_scheduling`

```rust
/// Tail helper for queue-mutation scenarios (add / retry-from-history / retry-existing).
/// Schedules next task if requested and emits queue-state-changed with scenario identity.
async fn complete_queue_mutation_scheduling(
    &self,
    request: ScheduleNextRequest,
    scenario: QueueMutationScenario,
) -> AppResult<()> {
    self.schedule_next_if_requested(request).await?;
    scenario.emit_queue_changed(self.events);
    Ok(())
}
```

三个调用点（QueueAdd / QueueRetryFromHistory / QueueRetryExisting）改用这个 helper。删除三个对应的 `complete_queue_add_scheduling` / `schedule_next_for_add_outcome_and_mark_history` / `schedule_next_for_existing_task_retry` 包装函数（它们都退化为 1 行调用 helper，没存在价值了）。同时删除被取代的 3 个 `mark_*` 别名（`mark_queue_add_scheduling_completed` / `mark_queue_retry_history_scheduling_completed` / `mark_queue_retry_existing_scheduling_completed`）。

### 3. 类型 B 的 9 个 mark 别名**保持不变**

第二轮讨论共识：它们是 988 行状态机的"读者路标"，inline 损失大于收益。

### 4. 不做的事

- ❌ 不抽新文件（同文件私有）
- ❌ 不引入新 port 或 pub API
- ❌ 不做 sub-orchestrator
- ❌ 不强行用 ScenarioOutcome 覆盖所有副作用场景（child-exit 流水线有 `mark_terminal_child_exit_failure` 这种带真实逻辑的 mark，不归并）

## 为什么不是更大的动作

Claude 原提议是"用 ScenarioOutcome 覆盖三场景（QueueStart / QueueAdd-Retry / ChildExit）的所有尾部副作用"。但代码核查显示**三场景的尾部副作用形态不一致**：

- **QueueAdd-Retry 尾部** = `schedule_next + queue_state_changed`（两步，同构）
- **ChildExit 尾部** = `clear_terminal_active_line + continue_unless_shutting_down(handle_X) + drive_child_exit_queue`（多步，含异步闭包 + 条件调度 + 错误警告）
- **QueueStart 尾部** = `reset_for_new_run + drive_queue_start`，且 pause/start 分支会 emit `queue_state_changed`，countdown-cancel 分支会 emit `shutdown_countdown_cancelled`（有事件副作用，但非同构尾部）

强行用一个 `ScenarioOutcome` 枚举统一这三种形态会让枚举变成"无所不包"的反模式。所以本 ADR **只覆盖形态同构的 QueueAdd-Retry 三场景**（类型 A），其余场景保持现状。

## 净效果

| 维度 | 改动前 | 改动后 |
|---|---|---|
| `mark_*` 别名 | 12 个 | 9 个（删除 3 个被 helper 取代的） |
| 中间包装函数 | 3 个（complete_queue_add_scheduling 等） | 0 个 |
| 副作用归并点 | 0 | 1（helper 内部，含 `match scenario` 消费） |
| 行数 | 988 | ~970（约 -18 行） |
| 调度决策归属 | 不变 | 不变 |

## 备选方案（已否决）

### A：裸 inline 9 个 mark 别名

第二轮 4 票反对。读者路标损失。

### B：Claude 原版候选 6（全场景 ScenarioOutcome）

三场景尾部形态不一致，强行统一枚举会变反模式。

### C：不做候选 6

5/6 票赞成做。类型 A 重复真实存在（3 处，codegraph_callers 确认）。

### D：删 enum，helper 只收 `ScheduleNextRequest`（Claude 评审提议）

被 3/6 票反对。场景身份作为 metrics 锚点有真实价值，立即消费避免 YAGNI 警告即可。

## Future Work

Kimi 第一轮指出文件里还有一类"一层套一层"的转发 wrapper 链值得后续单独归并（候选 7 范畴，不在本 ADR）：

- `schedule_next_and_start → schedule_next_internal → schedule_next_if_requested → try_schedule_next_and_start`

这条链是另一类形态同构的重复，但触发改动需要独立的 ADR（因为影响面更大，涉及 `try_schedule_next_and_start` 的多场景调用）。

## 评审轨迹

- 第二轮讨论：6 AI 投票（候选 5/6 方向裁决）
- ADR 草稿评审：6 AI 反馈（命名、消费、边界、遗漏核查）