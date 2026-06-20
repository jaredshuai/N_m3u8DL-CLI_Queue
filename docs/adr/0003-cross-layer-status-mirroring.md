# ADR-0003：同一个概念在每层各定义一份——以任务状态为例

- **状态**：已采纳
- **决策依据**：
  - `domain/task.rs`、`application/task_snapshot.rs`、`application/query_models.rs`、`adapters/task_record.rs`、`adapters/frontend_dto.rs`
  - 守护测试 `architecture_guard_static.rs:2822`（断言 `TaskStatus` 属 domain）、`:3374`（断言 `TaskStatusView` 属 application）

## 问题在哪

任务状态有四种：`Waiting`/`Downloading`/`Completed`/`Failed`。几乎每一层都要用到它。一种做法是**定义一个共享的类型，全局复用**；另一种是**每一层各定义一份自己的**。

## 决定怎么做

**每一层定义自己的一份，层与层之间用 `From` 显式转换。** 任务状态的完整链路：

| 层 | 类型 | 用途 | 要不要序列化 |
|---|---|---|---|
| domain | `TaskStatus` | 原始定义，带业务判断方法（`is_live_work`、`can_remove_from_queue`） | 不要（保持纯净） |
| application | `TaskStatusSnapshot` | 任务快照用（带运行时进度） | 不要 |
| application | `TaskStatusView` | 查询时用的只读视图 | 不要 |
| adapters | `StoredTaskStatus` | 存盘时用 | 要（`#[serde(rename_all="camelCase")]`） |
| adapters | `TaskStatusDto` | 传给前端时用 | 要（camelCase） |

每一对相邻层之间都有 `From` 实现，把状态从一层转成下一层（比如 `TaskStatus → TaskStatusSnapshot → TaskStatusView → TaskStatusDto`）。

## 为什么不让它们共用一份

- **隔离序列化的污染**：`StoredTaskStatus` 和 `TaskStatusDto` 需要序列化（加上 serde 派生和 camelCase），但 domain 的 `TaskStatus` 必须保持纯净（ADR-0001 不允许 domain 依赖 serde）。如果共用一个类型，domain 就会被迫染上序列化属性
- **隔离行为的差异**：domain 的 `TaskStatus` 带着**业务判断方法**（比如"是不是活跃任务"）；DTO 和快照不该暴露这些内部行为。共用一份，要么泄露内部逻辑，要么被迫把这些方法删掉
- **让每一层能各自演化**：哪天存储需要多一个中间态（比如"已暂停"），只在 `StoredTaskStatus` 里加就行，不用动业务语义
- **这个分工是被测试保护的**：守护测试明确断言 `TaskStatus` 归 domain、`TaskStatusView` 归 application。说明这是**有意为之**，不是写漏了

## 代价

- **明显的样板**：同样四个值，要定义 5 次，还要写 5 套 `From` 转换。不熟悉的人很容易当成"重复代码"想去合并——**这条 ADR 的作用之一就是阻止这种合并**
- **什么时候才该合并**：如果将来所有镜像的取值集合和行为长期、完全一致，可以考虑用一个 newtype 包住共享的内部 enum（就是给一个类型起个别名、加点约束的 Rust 写法）。但前提是评估清楚，会不会因此打破"domain 不依赖 serde"的边界
- **否决过的方案**：全局共享一个类型——会让 domain 依赖 serde，违反 ADR-0001
