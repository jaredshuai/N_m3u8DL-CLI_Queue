# 领域文档

工程类 skill 在探索代码库时，应如何消费本仓库的领域文档。

## 探索前，先读这些

- 根目录的 **`CONTEXT.md`**；或
- 根目录的 **`CONTEXT-MAP.md`**（如果存在）——它指向每个上下文各自的 `CONTEXT.md`。读取与主题相关的那些。
- **`docs/adr/`**——读取与你即将改动区域相关的 ADR。多上下文仓库中，还要检查 `src/<context>/docs/adr/` 是否有上下文范围内的决策。

如果这些文件尚不存在，**静默继续**。不要提示缺失，也不要主动建议创建。`/domain-modeling` skill（经由 `/grill-with-docs` 和 `/improve-codebase-architecture` 调用）会在术语或决策真正敲定时**懒生成**它们。

## 本仓库布局

**单一上下文**：根目录一份 `CONTEXT.md` + 一个 `docs/adr/`。

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-xxx.md
│   └── 0002-yyy.md
└── ...
```

> 注：本仓库根目录下同时存在 `cli/N_m3u8DL-CLI/`（旧 CLI，.NET Framework，已归拢进 `cli/`）和 `m3u8-queue-downloader/`（当前活跃 GUI，Tauri + Svelte）。由于日常只维护 GUI、旧 CLI 基本冻结，故采用单一上下文：`CONTEXT.md` 以 GUI 为主，顺带提及旧 CLI 的存在。若日后两个子项目都活跃，再切换到多上下文（`CONTEXT-MAP.md`）。

多上下文仓库（根目录存在 `CONTEXT-MAP.md` 时）的参考布局：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← 上下文专属决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表的词汇

当你的输出提到某个领域概念（issue 标题、重构提案、假设、测试名）时，使用 `CONTEXT.md` 中定义的术语。不要漂移到术语表明确规避的同义词。

如果你需要的概念尚不在术语表中，这是一个信号——要么你在发明项目不使用的语言（重新考虑），要么存在真实缺口（记录下来交给 `/domain-modeling`）。

## 标注 ADR 冲突

如果你的输出与某个现有 ADR 矛盾，明确点出而不是默默覆盖：

> _与 ADR-0007（事件溯源订单）冲突——但值得重开，因为……_
