# ADR-0001：后端分层架构（domain / application / ports / adapters / composition）

- **状态**：已采纳
- **决策依据**：`src-tauri/tests/architecture_guard_static.rs`（下面每条规则都对应一个测试）

## 为什么需要分层

下载器后端要同时打交道：外部子进程（N_m3u8DL-CLI）、文件存储、Tauri 框架、前端通信。如果业务逻辑直接碰这些东西，就会有两个麻烦：一是没法脱离运行时做测试，二是想换掉任何一个（比如换种存储方式）都得改业务代码。

分层的目的是把"做什么业务"和"用什么工具做"分开。

## 怎么分的

五层，各有明确职责，依赖只能朝一个方向走：

| 层 | 干什么 | 可以依赖 | 不能依赖 |
|---|---|---|---|
| **domain** | 业务核心：任务、状态、队列、重试规则 | 只能依赖标准库和 chrono | 任何其它层、任何框架 |
| **application** | 把业务步骤串起来，并声明需要哪些能力（端口） | domain + 端口约定 | 具体实现、Tauri、tokio、serde |
| **ports** | 端口约定的存放处（从用途上属于 application） | 只能依赖标准库 + domain | 具体实现、框架 |
| **adapters** | 端口的具体实现：真正读写文件、启动进程、调系统时钟 | 端口约定 + 各种框架 | — |
| **composition** | 装配区：创建所有实现、组装容器、注册 Tauri 命令 | 所有层 | — |

一句话：**composition 依赖 adapters，adapters 实现端口，application 用端口、依赖 domain，domain 谁都不依赖**。domain 是最里面、最干净的一层。

## 这些规则靠什么保证

不是靠口头约定，而是**编译时自动跑的测试**强制。文件 `architecture_guard_static.rs` 里的每个测试就是一条规则，举几个关键的：

- `domain_layer_has_no_outward_dependencies` —— domain 不能引用外面
- `application_layer_has_no_adapter_or_framework_dependencies` —— application 不能碰具体实现和框架
- `application_layer_uses_diagnostics_port_for_logging` —— 想打日志必须走端口，不能直接 `println`
- `ports_layer_has_no_adapter_or_framework_dependencies` —— 端口本身保持干净
- `app_bootstrap_is_composition_root_for_adapter_construction` —— 具体实现只能在 composition 里创建
- 还有 `*_guard_covers_every_declared_module` 一类：新增模块时会检查它有没有逃出规则的覆盖范围

想改架构，得先让这些测试过；测试就是规则的准确定义。

## 这样做的代价

- **好处**：domain 和 application 可以脱离 Tauri 单独测试；换掉某个实现（比如换存储后端）不影响业务逻辑
- **代价**：类型和转换的样板代码变多（见 ADR-0003）；composition 层的装配代码量不小
- **否决过的方案**：直接在 Tauri 命令里写业务逻辑——没法测试、和运行时绑死
