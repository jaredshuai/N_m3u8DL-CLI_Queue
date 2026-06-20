# ADR-0002：端口约定的归属——放在哪、从哪引用

- **状态**：已采纳
- **决策依据**：`src-tauri/src/application/mod.rs:40-57` 的 re-export 注释

## 问题在哪

端口约定（比如 `Clock`、`QueueRepository` 这种 trait）本质上是 **application 层提出的需求**——"我需要别人提供这种能力"。那它该放在哪个目录？

两种直觉放法：

- 放 `ports/`：目录名对得上
- 放 `application/`：它属于 application 的需求

## 决定怎么放

**文件物理上在 `src/ports/`，但从代码用途上属于 application 层**，通过 `application/mod.rs` 里的 re-export 桥接：

```rust
// application/mod.rs
pub(crate) use crate::ports::clock::Clock;
// 这样调用方写 crate::application::Clock，而不是 crate::ports::Clock
```

代码里的原话注释：`port traits are defined by the application layer (the contract it requires from adapters), so they belong here. The physical files remain in src/ports/ for now; these re-exports establish the correct dependency direction.`

## 为什么这么定

- **依赖方向表达得准**：代码里写 `application::Clock`，读起来就是"这是 application 层需要的能力"；写 `ports::Clock` 会让人误以为"这只是个叫 ports 的目录里的东西"
- **文件组织合理**：约定（`ports/`）和实现（`adapters/`）在文件树里挨着，方便对照
- **注释里那句 "for now" 是诚实标注**：承认物理位置是权宜之计，如果将来 re-export 反而让人困惑，可以把文件整体搬进 application

## 代价和风险

- **代价**：不熟悉的人会困惑"为什么 `ports/` 里的东西要从 `application` 引入"——这条 ADR 就是用来回答这个问题的
- **风险**：如果有人绕过 re-export 直接写 `ports::Clock`，语义就被削弱了。目前没有测试专门守护这一点
