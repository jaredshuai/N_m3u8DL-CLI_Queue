# ADR-0006：QueueRepository 窄 trait 分拆回退（扁平化合并）

- **状态**：已采纳（2026-06-23，架构评审候选 2）
- **决策依据**：`QueueRepository` 的三窄 trait 分拆（`QueueStateReader` / `QueueMutation` / `QueueRunLifecycle`）自引入以来始终无外部消费者，全部 12 个调用方只通过总和 trait `dyn QueueRepository` 使用

## 问题在哪

`ports/queue_repository.rs` 把 `QueueRepository` 拆成了一个总和 trait（4 行）+ 三个窄 trait + 三个 `Arc<T>` 转发 block，合计约 220 行代码。但：

1. **零窄 trait 外部消费者**：`dyn QueueStateReader`、`dyn QueueMutation`、`dyn QueueRunLifecycle` 均无引用——所有协作者通过 `&dyn QueueRepository` 或 `Arc<dyn QueueRepository>` 使用。分拆未产生任何实际的调用方分化。
2. **三个「空」seam**：`QueueRepository` 只有一个实现（`QueueManager`），场景上也没有按窄 trait 分离的组合需求（所有 orchestrator 需要同一组方法）。
3. **维护成本 > 收益**：三个 `Arc<T>` 转发 block（共 54 行）纯属脱裤子放气——如果保留单一 trait，只需一个 `impl<T> QueueRepository for Arc<T>` block（19 行）。空 seam 还要靠 guard test 锁定存在。
4. **ADR-0004 先例**：该 ADR 已判定 QueueSchedulingPorts 不应分拆（三场景共享同一调度执行流与状态机）。`QueueRepository` 的窄 trait 分拆与 ADR-0004 的核心理念矛盾——数据管理器（QueueManager）也共享同一组操作，强行分拆只是把大文件做成三个小 block + 三个 forwarding block，无实质内聚收益。

## 决定怎么做

### 1. 扁平化为单一 `QueueRepository` trait

三个窄 trait 的所有方法直接放在 `QueueRepository` 上，移除窄 trait（及它们的 `Send + Sync` bound 标注），保留总和 trait 名称。

### 2. 移除空白 blanket impl

```rust
impl<T> QueueRepository for T where
    T: QueueStateReader + QueueMutation + QueueRunLifecycle + Send + Sync
{ }
```

扁平化后 `QueueManager` 直接 `impl QueueRepository for QueueManager`。

### 3. 单一 `Arc<T>` 转发

```rust
impl<T> QueueRepository for Arc<T> where T: QueueRepository + ?Sized { … }
```

取代三个 54 行转发 block + 一个空白 blanket impl。

### 4. 适配器合并

`queue_manager.rs` 的三个 `impl` block（`QueueStateReader for QueueManager`、`QueueMutation for QueueManager`、`QueueRunLifecycle for QueueManager`）合并为一个 `impl QueueRepository for QueueManager`。

### 5. Guard test 更新

原断言锁定三个窄 trait impl block 存在的检查改为断言新的总和 trait impl block 存在且无窄 trait impl。

### 净效果

| 维度 | 改动前 | 改动后 | 差额 |
|---|---|---|---|
| trait 定义 | 4 个 trait + 1 blanket impl | 1 个 trait | −4 |
| Arc 转发代码量 | 54 行（3 block） | 19 行（1 block） | −35 |
| 适配器 impl block | 3 个 | 1 个 | −2 |
| 端口文件总行数 | ~205 行 | ~105 行 | −100 |

所有行为保持不变。

### 衰退预防

作为纯删除+重排改动，衰退风险极低。无新测试必要——现有 284 个测试通过即可确认。

## 什么没做（并说明理由）

- **没改成面向窄 trait 编程**：零外部消费者，强推只会徒增各处接线点的 edit noise。
- **没删除总和 trait 名称**：12 个文件用 `dyn QueueRepository`，改名会导致无价值的大规模 edit noise。
- **没动 `QueueRepositoryFuture` 别名**：本身是好的 type alias，无关联改动必要。

## 备选方案

### A：保持现状

不采纳。持续维护 220 行无用代码的维护成本 > 改造成本（纯删除，零行为变更，极低出错面）。

### B：删除窄 trait 的同时推 consumer 更新

即改所有 `&dyn QueueRepository` 引用为分窄 trait 的 ports struct。不采纳——消费者不需要这种分化（每个 orchestrator 需要多组方法），且会增加大量 edit noise 而不改善内聚性。