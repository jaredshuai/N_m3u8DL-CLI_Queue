# ADR-0009: 停止下载中任务（Per-Task Stop）

## Status

已采纳并实施（2026-06-24，commit `44afbf3`，PR #13；自 `app-v0.2.0` 起发布）

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

`QueueAggregate::stop_task` + `QueueTasks::stop_task`：仅允许 Downloading 状态的任务转入 Cancelled，`error_message = "Stopped by user"`，同时 `clear_current_task_if_matches(id)` 释放 current_task 占位。

### 3. 新增 `TaskLifecycleEvent::Cancelled` 变体

`application/task_process_events.rs` 新增 `Cancelled { id, error_message }`。生命周期 worker（`runtime_facade.rs`）匹配后路由到 `handle_cancelled_child_exit`。

### 4. 新增 port `TaskProcessSupervisor::terminate_task`

`ports/process_runner.rs` 新增 `terminate_task(task_id)`。`TaskRunner` adapter 实现：

- 无 PID 时按幂等成功处理并直接发送 `TaskLifecycleEvent::Cancelled`，避免死亡竞态中的在途 Completed/Failed 重新取得状态权威
- 找到 PID 时先把 `task_id` 加入 `cancelling: Arc<Mutex<HashSet<String>>>`，再 best-effort 调用 `kill_process(pid)`（Windows `taskkill /PID /T /F` / Unix `SIGTERM`）
- 仅当 `running_processes` 中仍是原 PID 时才移除，避免停止后立即重试时旧 waiter 删除新进程
- 直接发送 `TaskLifecycleEvent::Cancelled`；`spawn_wait_task` 消费 cancelling marker 后不再发送重复的 Completed/Failed
- 新进程注册时清理同 task_id 的旧 marker，避免停止后重试被永久静默

### 5. 新增 port `QueueRepository::stop_task`

`ports/queue_repository.rs` 新增 `stop_task(id)`。`QueueManager` adapter 实现：调用 `QueueAggregate::stop_task`，经 `application_stop_task_result` 映射 fold 成 `AppResult<()>`。

### 6. 新增 orchestrator `handle_cancelled_child_exit`

`application/queue_scheduling_orchestrator.rs` 新增 `handle_cancelled_child_exit` + `cancel_child_exit`：

- `clear_child_exit_terminal_active_line` 清终端
- `continue_child_exit_unless_shutting_down` 走 shutdown-gate（与 Completed/Failed 一致）
- 内部不调用 `prepare_task_failure`，只 emit warn 日志，并通过 `drive_child_exit_queue_and_report_finished("cancellation")` 继续调度/结束本轮
- 取消不触发自动关机倒计时：用户主动停止不等价于“全部任务自然完成”

### 7. 新增 `stop_task` Tauri command

`composition/queue_command_facade.rs::stop_task` 编排两步：

1. `queue_repository.stop_task(task_id)` — 队列侧先标 Cancelled + clear current_task
2. `process_supervisor.terminate_task(task_id)` — 杀进程 + 发 Cancelled 事件

**顺序很关键**：先 mark Cancelled，再 kill。这样即使 kill 后触发的 lifecycle 事件 race 进来，queue 侧已经是 Cancelled 状态，`continue_child_exit_unless_shutting_down` 里的 shutdown-gate 处理也不会再做错误状态转换。

`terminate_task` 的契约为 always-OK，kill 在 adapter 内 best-effort 执行；因此 mark 成功后不存在“kill 返回错误但队列已是 Cancelled”的半完成命令结果。cancelling marker 负责阻止 waiter 再发送 Completed/Failed。

**不用 `begin_shutdown()` 全局闸**：原计划复用 `shutting_down` flag 禁止新启动，副作用太大（杀一个任务就把整个队列的 spawn 都禁了）。改成 per-task `cancelling` 集合即可。

### 8. 前端

- `TaskCard.svelte`：当 `task.status === 'downloading'`，在卡片 actions 区显示 **⏹ 停止** 按钮；点击调用 `stopTask(task.id)`。新增 `stopping` 瞬态状态防连点（按钮显示"停止中..."并 disabled）。
- `TaskCard.svelte`：新增 `cancelled` 状态徽章（灰色）和 error-msg 显示（同样灰色调）。
- `queue-store.js`：新增 `stopTask(taskId)` 导出，`invoke('stop_task')` + 落地 `loadQueueState()` 双保险刷新。

### 9. 边界情况

| 场景 | 行为 |
|---|---|
| 进程已自然退出（死亡竞态） | `terminate_task` 找不到 PID → `Ok(())`，仍发送 Cancelled 事件；queue 侧已是 Cancelled，状态权威 |
| 用户连点两次停止 | 第一次 `stopping=true` 防连点；第二次直接 return |
| 停止后立刻重试 | Cancelled → retry_task 允许 → Waiting；新进程注册会清旧 marker，旧 waiter 的 PID 匹配保护不会删除新 PID |
| 杀进程与 lifecycle worker 竞态 | `cancelling` 集合保证 `spawn_wait_task` 不会发重复的 Completed/Failed |
| OS kill 失败 | adapter 仍返回 `Ok(())` 并保留 marker；队列状态以 Cancelled 为权威，不把底层 kill 错误暴露为半完成命令 |

## Consequences

**正向：**

- 用户能立即中止卡住的下载任务，UI 反馈即时
- domain 新增 Cancelled 语义清晰，与 Failed 区分（不重试）
- 复用 shutdown-gate 模式，不引入新机制
- 单任务粒度，不影响队列里其他任务

**负向 / 取舍：**

- 新增 1 个 domain 状态变体，所有 `match TaskStatus` 的位置都要更新（已修复编译错误）
- TaskRunner 新增 `cancelling` 字段，与 `shutting_down` 平行（语义独立）
- Windows 下杀进程用 `taskkill /F`（强制），不做 graceful-then-force 升级；kill 失败按 best-effort 处理，不向 command 调用方返回错误
- Cancelled 不进入历史，与 Failed 区别对待——如果未来想把 Cancelled 也归入失败历史，再改 `history_status_from_snapshot` 即可

## Alternatives considered

### A. 不新增 Cancelled 状态，复用 Failed

被否决。Failed 会触发 retry policy，把杀掉的任务自动重排队——与用户意图相反。如果再 hack 一个"不要 retry"的 sentinel error_message，跨层用字符串传递意图，比新增 enum 变体更脏。

### B. sentinel error_message 跳过 retry

被否决。同上，跨层用字符串做控制流不优雅。

### C. 复用 shutdown-gate 模式（`shutting_down` flag）

被否决。`begin_shutdown` 是全局闸，杀一个任务会把整个队列的 spawn 都禁了。`cancelling` 是 per-task 集合，更精确。
