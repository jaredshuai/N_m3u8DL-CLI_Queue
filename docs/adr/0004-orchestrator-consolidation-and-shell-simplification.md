# ADR-0004：编排器收敛与包装器简化规范

- **状态**：已采纳
- **决策依据**：`src-tauri/src/application/queue_scheduling_orchestrator.rs` 与 `src-tauri/tests/architecture_guard_static.rs` 之间的设计争议与对立评审意见评估

## 问题在哪

在 commit `45c5e8b` 的大规模架构重构中，我们删除了 18 个无逻辑的 use-case 壳文件，整合为了中心化的 Orchestrator。然而，这导致了两个极端现象：
1. **中心编排器体量膨胀**：`QueueSchedulingPorts`（位于 `src-tauri/src/application/queue_scheduling_orchestrator.rs`）成为持有 9 个注入 Port、长度近 900 行的庞大协调器，导致部分静态分析提示过载。
2. **边界包装器（Shells）造成认知阻碍**：仍保留了 `TaskLifecyclePorts` 与 `QueueStartPorts` 等几乎不含任何业务逻辑的透传壳（这两个文件已在本次决策落地时删除）。它们的存在，是迫于静态架构守护测试对类/结构体命名的微观硬编码限制。

团队需要对以下两点做出决策：
- 是否应该为了“模块单一职责/行数少”而强行将核心调度器物理拆分为不相交的文件？
- 是否应当废除无实际业务逻辑的透传 Wrapper 壳？

## 决定怎么做

本决策确立了以下规范：

1. **坚持调度核心中枢的物理内聚性**：
   - 坚决反对仅因行数或职责场景多，而将 `QueueSchedulingPorts` 物理拆分为多个独立文件。
   - 理由：QueueStart、QueueAdd/Retry 与 ChildExit 全都在控制逻辑上高度依赖**同一个调度执行流与状态机**。强行拆分会带来大量的网状跨模块互调，极度增加编译期生命周期所有权心智负担。
   - **允许局部委托**：未来如需减负，应通过内部构造 Sub-ports/Sub-orchestrators（例如已有的 `TerminalHistoryPorts`）进行局部拆解与委托，而保持整体编排生命周期一致。

2. **简化/废除零逻辑透传包装壳**：
   - 彻底删除无逻辑透传壳（如 `TaskLifecyclePorts`，文件已删除）。
   - 让 Facade / Composition 装配层直接实例化并调用 `QueueSchedulingPorts`，减少无意义的间接跳转。

3. **静态守护测试规则松绑**：
   - 修改 `architecture_guard_static.rs`（`src-tauri/tests/`）。
   - 架构测试应重点关注"依赖方向"（Composition -> Adapters -> Ports <- Application -> Domain），禁止过度断言具体的类名、特定构造结构以及纯语法占位符的显式匹配。

## 为什么这么定

- **开发效率与 Locality 提升**：删除套壳后，开发人员定位逻辑无需连续穿透 3 个以上文件，降低了日常排查故障的追踪成本。
- **状态一致性保障**：核心调度引擎和队列启动/退出等强状态关联流程继续物理内聚在 `QueueSchedulingPorts`，规避了状态不同步或网状调用带来的安全隐患。
- **避免教条的静态守护**：守护测试应当是重构时的防护网，而不是阻碍合理架构简化的枷锁。

## 代价和风险

- **代价**：`QueueSchedulingPorts` 的依赖总数较多（9个），在单元测试时需要构造更多的 mock 依赖（但实际已有相应的 TestHelper/DependencyGraph 封装降低该阻碍）。
- **风险**：松绑静态守护测试可能在短期内让开发人员对用例结构的把握失去绝对一致的编译时强制规范，需在 Code Review 阶段通过设计审查弥补。
