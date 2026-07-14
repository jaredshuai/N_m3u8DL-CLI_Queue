# ADR-0009: 停止下载中任务（Per-Task Stop）

## Status

已采纳并实施（2026-06-24，commit `44afbf3`，PR #13；自 `app-v0.2.0` 起发布；2026-07-14 完成 OS 终止语义加固）

## 2026-07-14 OS 终止语义加固

原实现把 cancellation marker 只绑定到 `task_id`，并在 OS kill 命令返回后立即释放队列槽、发送 Cancelled。复核确认这不能表达真实进程生命周期：Unix `SIGTERM` 只表示信号已发送，kill 失败时进程仍可能存活，立即重试还会让新旧 waiter 争用同一 marker。

加固后的协议为：

1. `TaskProcessSupervisor` 先 claim 当前注册的具体进程 generation；claim 之后 waiter 即使先退出，也会等待 commit/abort 决议。
2. claim 成功后持久化 Cancelled，但保留 `current_task`，继续占用串行执行槽；持久化失败则 abort claim，waiter 恢复原始 Completed/Failed。
3. 队列持久化成功后执行 OS 终止尝试并 commit claim；即使 kill 返回错误，已持久化的取消意图也不再降级。Cancelled 事件改由匹配 generation 的 waiter 在 `child.wait()` 真正返回后发送。
4. `handle_cancelled_child_exit` 先持久化 `finalize_task_cancellation` 清除 current slot，再调度下一任务。
5. kill 失败不再吞掉：命令返回错误，但任务保持 Cancelled + current，防止并发下载；用户可再次停止，进程以后自然退出时仍按 Cancelled 收尾。
6. Windows 使用 `taskkill /PID /T /F` 并等待 waiter 确认；Unix 把 CLI 放进独立 process group，先向整组发 SIGTERM，3 秒未退出再发 SIGKILL，并再次等待确认。

## Context

当前 GUI 对卡住的下载任务（status=downloading）没有任何停止手段：

- `pause_queue` 命令只切 `QueueRunStatus::Paused`，**不杀进程**，等它自然结束；UI 显示"收尾中..."。若 N_m3u8DL-CLI 子进程真的卡住（例如对方 CDN 中断、TCP 半连接），队列会无限等下去。
- `remove_task` 拒绝删除 `Downloading` 状态的任务（`domain/task.rs::can_remove_from_queue` 仅允许 Waiting/Failed）。
- 唯一杀子进程的路径是 `exit_application` → `terminate_all_running_processes`，但那是退出整个 app，且杀全部。

用户需要一个按钮，能立即杀掉卡住的 CLI 子进程，把任务恢复到可操作状态（可删除、可重试）。

## Decision

### 1. 新增 `TaskStatus::Cancelled` domain 变体

`domain/task.rs` 新增 `Cancelled`。行为规则：

- `can_remove_from_queue()` = true（同 Failed）
- `is_live_work()` = false（不会阻塞 `finish_run_if_idle`）
- 手动 Retry 允许（Cancelled 可重试回 Waiting，跟 Failed 一致）
- 不进入 pending history（Cancelled 不属于 terminal history 之一）
- 不经过 `prepare_task_failure` 的 retry policy——这是用户主动取消，不是失败

### 2. 新增 domain `stop_task(id)`

`QueueAggregate::stop_task` + `QueueTasks::stop_task`：仅允许 Downloading 状态的任务转入 Cancelled，`error_message = "Stopped by user"`。Cancelled 先保留 current_task 占位，直到匹配的进程 waiter 确认退出；`finalize_task_cancellation(id)` 才释放该槽。仍占槽的 Cancelled 任务不能重试或删除，重复 stop 按幂等成功处理。

### 3. 新增 `TaskLifecycleEvent::Cancelled` 变体

`application/task_process_events.rs` 新增 `Cancelled { id, error_message }`。生命周期 worker（`runtime_facade.rs`）匹配后路由到 `handle_cancelled_child_exit`。

### 4. 新增 port `TaskProcessSupervisor` generation claim 协议

`ports/process_runner.rs` 通过 `claim_task_termination` / `abort_task_termination` / `terminate_claimed_task` 表达三阶段协议。`TaskRunner` adapter 实现：

- `running_processes` 的每个 entry 带单调 generation、claim 状态和退出通知，不再使用按 task_id 共享的 HashSet marker
- claim 与 waiter 清理在同一个注册表边界上按 generation 匹配；旧 waiter 不能删除或静默新进程
- 无注册进程时返回 `AlreadyExited`，停止操作输给已经发生的自然退出，原 Completed/Failed 事件继续取得权威
- claim 后若队列持久化失败，abort 让 waiter 恢复原始 Completed/Failed；成功后执行 OS 终止尝试并 commit，kill 错误仍返回调用方
- 同一 generation 已有 claim 时返回 `AlreadyClaimed`，并发重复命令不能 abort 原 claim；已 committed 的进程仍可再次 claim 以重试 OS kill
- Cancelled 事件由 waiter 在 `child.wait()` 返回后发送，OS 命令返回本身不再被视为退出确认

### 5. 新增 port `QueueRepository::stop_task`

`ports/queue_repository.rs` 新增 `stop_task(id)`。`QueueManager` adapter 实现：调用 `QueueAggregate::stop_task`，经 `application_stop_task_result` 映射 fold 成 `AppResult<()>`。

### 6. 新增 orchestrator `handle_cancelled_child_exit`

`application/queue_scheduling_orchestrator.rs` 新增 `handle_cancelled_child_exit` + `cancel_child_exit`：

- `clear_child_exit_terminal_active_line` 清终端
- `finalize_task_cancellation` 持久化释放匹配的 current slot；缺失/非当前事件幂等忽略
- `continue_child_exit_unless_shutting_down` 走 shutdown-gate（与 Completed/Failed 一致）
- 内部不调用 `prepare_task_failure`，只 emit warn 日志，并通过 `drive_child_exit_queue_and_report_finished("cancellation")` 继续调度/结束本轮
- 取消不触发自动关机倒计时：用户主动停止不等价于“全部任务自然完成”

### 7. 新增 `stop_task` Tauri command

`composition/queue_command_facade.rs::stop_task` 编排三阶段协议：

1. `claim_task_termination(task_id)` — 原子 claim 当前进程 generation；无进程则自然退出胜出并幂等返回
2. `queue_repository.stop_task(task_id)` — 持久化 Cancelled，但保留 current slot；失败时 abort claim
3. `terminate_claimed_task(claim)` — 请求 OS 终止、等待 waiter 确认；waiter 随后发 Cancelled 事件完成队列收尾

这个顺序同时关闭两种半完成状态：队列持久化失败时尚未杀进程且 waiter 恢复自然事件；kill 失败时队列虽已是 Cancelled，但 current slot 仍被占用，不会启动第二个下载进程。

**不用 `begin_shutdown()` 全局闸**：原计划复用 `shutting_down` flag 禁止新启动，副作用太大（杀一个任务就把整个队列的 spawn 都禁了）。generation claim 只冻结目标进程的 waiter 决议，队列槽则由持久化的 `current_task` 精确占用。

### 8. 前端

- `TaskCard.svelte`：Downloading 显示停止按钮；Cancelled 且仍是 `currentTaskId` 时显示“停止中”，保留停止按钮供 OS kill 失败后重试，并隐藏重试/删除操作直到进程退出。
- `TaskCard.svelte`：新增 `cancelled` 状态徽章（灰色）和 error-msg 显示（同样灰色调）。
- `queue-store.js`：新增 `stopTask(taskId)` 导出，`invoke('stop_task')` + 落地 `loadQueueState()` 双保险刷新。

### 9. 边界情况

| 场景 | 行为 |
|---|---|
| 进程已自然退出（死亡竞态） | claim 找不到注册进程 → `AlreadyExited`，不再改成 Cancelled；已发生的自然 Completed/Failed 胜出 |
| 用户连点两次停止 | 前端以 `stopping` 防连点；后端重复 claim 返回 `AlreadyClaimed`，不干扰原操作 |
| 停止后立刻重试/删除 | Cancelled 仍占 current slot 时拒绝；waiter 确认退出并 finalize 后才允许 |
| 杀进程与 lifecycle worker 竞态 | generation claim 使 waiter 在 commit/abort 前等待，且只清理自己的注册记录 |
| OS kill 失败 | 返回错误；Cancelled + current 保持安全阻塞，可再次停止，真实退出后自动 finalize |
| Unix 子进程树 | CLI 使用独立 process group；TERM 发给整组，超时后 KILL 整组 |

## Consequences

**正向：**

- 用户能立即中止卡住的下载任务，UI 反馈即时
- domain 新增 Cancelled 语义清晰，与 Failed 区分（不重试）
- generation claim 与 current-slot 占用共同保证停止期间不会并发启动下一进程
- 单任务粒度，不影响队列里其他任务

**负向 / 取舍：**

- 新增 1 个 domain 状态变体，所有 `match TaskStatus` 的位置都要更新（已修复编译错误）
- TaskRunner 为每个运行进程保存 generation + watch 状态；共享 marker 集合已删除
- Windows 下杀进程用 `taskkill /T /F`（强制）并确认退出；Unix 使用 process-group TERM→KILL 升级
- kill 失败会显式返回，队列通过保留 current slot 维持串行不变量
- Cancelled 不进入历史，与 Failed 区别对待——如果未来想把 Cancelled 也归入失败历史，再改 `history_status_from_snapshot` 即可

## Alternatives considered

### A. 不新增 Cancelled 状态，复用 Failed

被否决。Failed 会触发 retry policy，把杀掉的任务自动重排队——与用户意图相反。如果再 hack 一个"不要 retry"的 sentinel error_message，跨层用字符串传递意图，比新增 enum 变体更脏。

### B. sentinel error_message 跳过 retry

被否决。同上，跨层用字符串做控制流不优雅。

### C. 复用 shutdown-gate 模式（`shutting_down` flag）

被否决。`begin_shutdown` 是全局闸，杀一个任务会把整个队列的 spawn 都禁了。generation claim 只约束目标进程，持久化的 current slot 只阻塞该轮串行调度，粒度更精确。
