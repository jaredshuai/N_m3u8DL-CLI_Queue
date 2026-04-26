<script>
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import {
    CLI_OUTPUT_PAGE_SIZE,
    prependCliOutputPage,
  } from './cli-output.js';
  import {
    beginTerminalStateLoad,
    capRenderedTerminalLines,
    MAX_RENDERED_TERMINAL_LINES,
    createTerminalLoadState,
    resolveTerminalActiveLine,
    shouldApplyTerminalResponse,
    shouldStartTerminalStateLoad,
  } from './cli-console.js';
  import { displayProgressPercent } from './progress.js';

  let {
    task,
    onClose,
    overlay = false,
    liveCommittedLines = [],
    liveActiveLine = '',
    hasLiveActiveLine = false,
  } = $props();

  let cliPanel = $state(null);
  let committedLines = $state([]);
  let activeLine = $state('');
  let cliOutputOffset = $state(0);
  let cliOutputTotal = $state(0);
  let cliOutputHasMoreBefore = $state(false);
  let cliOutputLoading = $state(false);
  let cliOutputError = $state('');
  let autoStickToBottom = $state(true);
  let terminalLoadState = createTerminalLoadState();

  let statusKey = $derived(
    task?.status === 'downloading' ? 'down' :
    task?.status === 'waiting' ? 'wait' :
    task?.status === 'completed' ? 'done' :
    task?.status === 'failed' ? 'fail' : 'wait'
  );

  let statusLabel = $derived(
    task?.status === 'downloading' ? '运行中' :
    task?.status === 'waiting' ? '等待中' :
    task?.status === 'completed' ? '已完成' :
    task?.status === 'failed' ? '失败' : '终端'
  );

  let displayTitle = $derived(
    task?.saveName || task?.url || 'CLI 实况'
  );

  let progressPct = $derived(displayProgressPercent(task?.progress));

  // Only use the new terminal stream model; do not merge any legacy logLines.
  let mergedCommittedLines = $derived(mergeCommitted(committedLines, liveCommittedLines ?? []));
  let renderedCommittedLines = $derived(capRenderedTerminalLines(mergedCommittedLines));
  let hiddenRenderedLineCount = $derived(
    Math.max(0, mergedCommittedLines.length - renderedCommittedLines.length)
  );

  let displayActiveLine = $derived(
    hasLiveActiveLine ? (liveActiveLine ?? '') : resolveTerminalActiveLine(task, activeLine)
  );

  let totalLineCount = $derived(mergedCommittedLines.length + (displayActiveLine ? 1 : 0));
  let displayLineCount = $derived(
    Math.max(cliOutputTotal ?? 0, mergedCommittedLines.length) + (displayActiveLine ? 1 : 0)
  );

  function mergeCommitted(persisted, live) {
    if (persisted.length === 0) return live;
    if (live.length === 0) return persisted;

    const maxOverlap = Math.min(persisted.length, live.length);
    for (let overlap = maxOverlap; overlap > 0; overlap -= 1) {
      const pSuffix = persisted.slice(persisted.length - overlap);
      const lPrefix = live.slice(0, overlap);
      if (pSuffix.length === lPrefix.length && pSuffix.every((v, i) => v === lPrefix[i])) {
        return [...persisted, ...live.slice(overlap)];
      }
    }

    return [...persisted, ...live];
  }

  async function loadTerminalState(taskId, taskStatus) {
    terminalLoadState = beginTerminalStateLoad(terminalLoadState, {
      id: taskId,
      status: taskStatus,
    });
    const requestId = terminalLoadState.requestId;
    cliOutputLoading = true;
    cliOutputError = '';
    try {
      const state = await invoke('get_cli_terminal_state', {
        taskId,
        limit: CLI_OUTPUT_PAGE_SIZE,
      });
      if (!shouldApplyTerminalResponse(requestId, terminalLoadState.requestId)) return;
      committedLines = state.committedLines ?? [];
      activeLine = state.activeLine ?? '';
      cliOutputOffset = state.offset ?? 0;
      cliOutputTotal = state.total ?? committedLines.length;
      cliOutputHasMoreBefore = state.hasMoreBefore ?? false;
    } catch (err) {
      if (!shouldApplyTerminalResponse(requestId, terminalLoadState.requestId)) return;
      cliOutputError = String(err);
      committedLines = [];
      activeLine = '';
      cliOutputOffset = 0;
      cliOutputTotal = 0;
      cliOutputHasMoreBefore = false;
    } finally {
      if (shouldApplyTerminalResponse(requestId, terminalLoadState.requestId)) {
        cliOutputLoading = false;
      }
    }
  }

  async function loadEarlierCliOutput() {
    if (cliOutputLoading || !cliOutputHasMoreBefore || !task?.id) return;

    cliOutputLoading = true;
    cliOutputError = '';
    const nextLimit = Math.min(CLI_OUTPUT_PAGE_SIZE, cliOutputOffset);
    const nextOffset = Math.max(0, cliOutputOffset - nextLimit);

    try {
      const page = await invoke('get_cli_output_page', {
        taskId: task.id,
        offset: nextOffset,
        limit: nextLimit,
      });
      committedLines = prependCliOutputPage(committedLines, page);
      cliOutputOffset = page.offset ?? nextOffset;
      cliOutputTotal = page.total ?? cliOutputTotal;
      cliOutputHasMoreBefore = page.hasMoreBefore ?? false;
    } catch (err) {
      cliOutputError = String(err);
    } finally {
      cliOutputLoading = false;
    }
  }

  function handleScroll() {
    if (!cliPanel) return;
    const remaining = cliPanel.scrollHeight - cliPanel.scrollTop - cliPanel.clientHeight;
    autoStickToBottom = remaining < 24;
  }

  function scrollToBottom() {
    if (!cliPanel) return;
    cliPanel.scrollTop = cliPanel.scrollHeight;
    autoStickToBottom = true;
  }

  $effect(() => {
    if (!shouldStartTerminalStateLoad(task, terminalLoadState)) return;
    committedLines = [];
    activeLine = '';
    cliOutputOffset = 0;
    cliOutputTotal = 0;
    cliOutputHasMoreBefore = false;
    cliOutputError = '';
    autoStickToBottom = true;
    loadTerminalState(task.id, task.status);
  });

  $effect(() => {
    const _trigger = totalLineCount;
    if (!cliPanel || !autoStickToBottom) return;

    tick().then(() => {
      if (cliPanel && autoStickToBottom) {
        cliPanel.scrollTop = cliPanel.scrollHeight;
      }
    });
  });
</script>

<section class:overlay class="cli-console-shell">
  <div class="cli-console-header">
    <div class="cli-console-title">
      <strong>{displayTitle}</strong>
      <span>{task?.url}</span>
    </div>
    <div class="cli-console-actions-row">
      <button class="meta-btn" onclick={scrollToBottom}>回到底部</button>
      <button class="meta-btn close" onclick={onClose}>关闭</button>
    </div>
  </div>

  <div class="cli-console-statusbar">
    <div class="cli-console-meta">
      <span class="meta-badge {statusKey}">{statusLabel}</span>
      {#if task?.status === 'downloading'}
        <span class="meta-pill meta-progress">{progressPct}%</span>
        {#if task?.speed}
          <span class="meta-pill meta-metric">{task.speed}</span>
        {/if}
        {#if task?.threads}
          <span class="meta-pill meta-metric">线程 {task.threads}</span>
        {/if}
      {/if}
    </div>
    <div class="cli-console-summary">
      <span class="meta-pill subtle">{displayLineCount} 行</span>
    </div>
  </div>

  {#if cliOutputHasMoreBefore}
    <div class="cli-console-load-row">
      <button class="meta-btn" onclick={loadEarlierCliOutput} disabled={cliOutputLoading}>
        {cliOutputLoading ? '加载中...' : '加载更早输出'}
      </button>
    </div>
  {/if}

  <div class="cli-console-body" bind:this={cliPanel} onscroll={handleScroll}>
    {#if cliOutputError}
      <div class="cli-line cli-error">CLI 实况加载失败：{cliOutputError}</div>
    {/if}
    {#if mergedCommittedLines.length > 0 || displayActiveLine}
      {#if hiddenRenderedLineCount > 0}
        <div class="cli-line cli-truncated">
          为保持界面响应，仅渲染最近 {MAX_RENDERED_TERMINAL_LINES} 行；更早输出仍保留在分页总数中。
        </div>
      {/if}
      {#each renderedCommittedLines as line}
        <div class="cli-line">{line}</div>
      {/each}
      {#if displayActiveLine}
        <div class="cli-line cli-active-line">{displayActiveLine}</div>
      {/if}
    {:else if cliOutputLoading}
      <div class="cli-line cli-empty">CLI 实况加载中...</div>
    {:else}
      <div class="cli-line cli-empty">暂无 CLI 实况</div>
    {/if}
  </div>
</section>

<style>
  .cli-console-shell {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    height: 320px;
    margin: 0 16px 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    background:
      linear-gradient(180deg, rgba(10, 12, 16, 0.98), rgba(4, 6, 9, 0.98)),
      radial-gradient(circle at top right, rgba(250, 204, 21, 0.08), transparent 35%);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }

  .cli-console-shell.overlay {
    height: 100%;
    margin: 0;
    border-radius: 18px;
    border-color: rgba(255, 255, 255, 0.1);
    box-shadow:
      0 22px 52px rgba(0, 0, 0, 0.42),
      inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .cli-console-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 16px;
    padding: 12px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.02);
  }

  .cli-console-title {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .cli-console-title strong {
    font-size: 13px;
    color: var(--color-text-main);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cli-console-title span {
    font-size: 11px;
    color: var(--color-text-disabled);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cli-console-statusbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    font-size: 11px;
    color: var(--color-text-disabled);
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    background: rgba(255, 255, 255, 0.015);
  }

  .cli-console-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex-wrap: nowrap;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .cli-console-meta::-webkit-scrollbar {
    display: none;
  }

  .cli-console-actions-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: nowrap;
    flex-shrink: 0;
  }

  .meta-badge,
  .meta-pill {
    padding: 4px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 700;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: var(--color-text-secondary);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .meta-badge.down {
    color: var(--color-status-down);
  }

  .meta-badge.done {
    color: var(--color-status-done);
  }

  .meta-badge.fail {
    color: var(--color-status-fail);
  }

  .meta-badge.wait {
    color: var(--color-status-wait);
  }

  .meta-progress {
    min-width: 56px;
    justify-content: center;
    text-align: center;
  }

  .meta-metric {
    min-width: 88px;
  }

  .subtle {
    color: var(--color-text-disabled);
    background: rgba(255, 255, 255, 0.015);
  }

  .cli-console-summary {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-shrink: 0;
  }

  .meta-btn {
    padding: 6px 10px;
    min-width: 78px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: var(--color-text-main);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .meta-btn.close {
    color: var(--color-status-fail);
  }

  .cli-console-load-row {
    display: flex;
    justify-content: flex-start;
    gap: 12px;
    padding: 8px 14px;
    font-size: 11px;
    color: var(--color-text-disabled);
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .cli-console-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 0 14px;
    font-family: 'Cascadia Code', 'SF Mono', Consolas, monospace;
    font-size: 12px;
    line-height: 1.55;
    background:
      linear-gradient(180deg, rgba(255,255,255,0.015), rgba(255,255,255,0)),
      repeating-linear-gradient(
        180deg,
        rgba(255,255,255,0.012) 0,
        rgba(255,255,255,0.012) 1px,
        transparent 1px,
        transparent 26px
      );
  }

  .cli-line {
    padding: 0 14px;
    color: #d1d7df;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .cli-line::before {
    content: '› ';
    color: rgba(250, 204, 21, 0.55);
  }

  .cli-active-line {
    color: #facc15;
  }

  .cli-active-line::before {
    content: '▸ ';
    color: rgba(250, 204, 21, 0.85);
  }

  .cli-empty {
    color: var(--color-text-disabled);
    font-style: italic;
  }

  .cli-error {
    color: var(--color-status-fail);
  }

  .cli-truncated {
    color: var(--color-text-disabled);
    font-style: italic;
  }

  @media (max-width: 900px) {
    .cli-console-header,
    .cli-console-statusbar {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px;
    }
  }

  @media (max-width: 640px) {
    .cli-console-header {
      gap: 10px;
    }

    .cli-console-actions-row {
      gap: 6px;
    }

    .meta-btn {
      min-width: 70px;
      padding: 6px 8px;
    }

    .cli-console-statusbar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .cli-console-summary {
      justify-content: flex-start;
    }
  }
</style>
