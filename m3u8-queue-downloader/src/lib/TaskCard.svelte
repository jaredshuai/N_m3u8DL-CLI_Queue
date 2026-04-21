<script>
  import { invoke } from '@tauri-apps/api/core';
  import { trackSessionTask } from './stores.js';

  let { task, draggable = false, historical = false } = $props();

  let showLog = $state(false);

  let statusKey = $derived(
    task.status === 'downloading' ? 'down' :
    task.status === 'waiting' ? 'wait' :
    task.status === 'completed' ? 'done' :
    task.status === 'failed' ? 'fail' : 'wait'
  );

  let borderColor = $derived(
    statusKey === 'down' ? 'var(--color-status-down)' :
    statusKey === 'done' ? 'var(--color-status-done)' :
    statusKey === 'fail' ? 'var(--color-status-fail)' :
    'var(--color-status-wait)'
  );

  let statusLabel = $derived(
    task.status === 'downloading' ? '下载中' :
    task.status === 'waiting' ? '等待中' :
    task.status === 'completed' ? '已完成' :
    task.status === 'failed' ? '失败' : task.status
  );

  let displayTitle = $derived(
    task.saveName || (task.status === 'waiting' ? '自动识别中...' : task.url)
  );

  async function handleRemove() {
    try {
      await invoke('remove_task', { taskId: task.id });
    } catch (err) {
      console.error('Failed to remove task:', err);
    }
  }

  async function handleRetry() {
    try {
      const retriedTask = await invoke('retry_task', { taskId: task.id });
      trackSessionTask(retriedTask.id);
    } catch (err) {
      console.error('Failed to retry task:', err);
    }
  }

  function toggleLog() {
    showLog = !showLog;
  }
</script>

<div
  class="task-card"
  style="border-left: 3px solid {borderColor};"
  class:downloading={statusKey === 'down'}
  class:completed={statusKey === 'done'}
>
  <div class="card-main">
    <!-- Drag handle for waiting tasks -->
    {#if draggable}
      <div class="drag-handle" title="拖动排序">⠿</div>
    {/if}

    <div class="card-content">
      <!-- Title row -->
      <div class="title-row">
        <span class="task-title">{displayTitle}</span>
        <span class="status-badge {statusKey}">{statusLabel}</span>
      </div>

      <!-- URL row -->
      <div class="task-url" title={task.url}>{task.url}</div>

      <!-- Progress bar (downloading only) -->
      {#if statusKey === 'down'}
        <div class="progress-bar">
          <div class="progress-fill" style="width: {Math.round(task.progress ?? 0)}%"></div>
        </div>
        <div class="progress-info">
          <span class="progress-pct">{Math.round(task.progress ?? 0)}%</span>
          {#if task.speed}
            <span class="speed">{task.speed}</span>
          {/if}
          {#if task.threads}
            <span class="threads">线程 {task.threads}</span>
          {/if}
        </div>
      {/if}

      <!-- Error message for failed tasks -->
      {#if statusKey === 'fail' && task.errorMessage}
        <div class="error-msg">{task.errorMessage}</div>
      {/if}

      <!-- Output path for completed tasks -->
      {#if statusKey === 'done' && task.outputPath}
        <div class="output-path" title={task.outputPath}>📁 {task.outputPath}</div>
      {/if}

      <!-- Action buttons -->
      <div class="task-actions">
        {#if statusKey === 'wait'}
          <button class="action-btn danger" onclick={handleRemove} title="删除">✕</button>
        {:else if statusKey === 'down'}
          <button class="action-btn" onclick={toggleLog} title="查看日志">📄</button>
        {:else if statusKey === 'fail'}
          <button class="action-btn accent" onclick={handleRetry} title="重试">🔄</button>
          {#if !historical}
            <button class="action-btn danger" onclick={handleRemove} title="删除">✕</button>
          {/if}
        {/if}
      </div>
    </div>
  </div>

  <!-- Expandable log area -->
  {#if showLog && (statusKey === 'down' || (task.logLines && task.logLines.length > 0))}
    <div class="log-area fade-in">
      {#if task.logLines && task.logLines.length > 0}
        {@const logLines = task.logLines}
        {#each logLines as line, i}
          <div class="log-line">{line}</div>
        {/each}
      {:else}
        <div class="log-line log-empty">暂无日志输出</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .task-card {
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: 8px;
    box-shadow: var(--card-inner-shadow, inset 0 1px 0 rgba(255,255,255,0.05));
    transition: background 0.15s, box-shadow 0.15s;
  }

  .task-card:hover {
    background: #181c24;
  }

  .card-main {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .drag-handle {
    color: var(--color-text-disabled);
    font-size: 16px;
    cursor: grab;
    user-select: none;
    padding: 2px 2px 0 0;
    line-height: 1;
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  .card-content {
    flex: 1;
    min-width: 0;
  }

  .title-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .task-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--color-text-main);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .status-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--radius-pill);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status-badge.wait {
    background: rgba(100, 116, 139, 0.15);
    color: var(--color-status-wait);
  }

  .status-badge.down {
    background: rgba(234, 179, 8, 0.15);
    color: var(--color-status-down);
  }

  .status-badge.done {
    background: rgba(16, 185, 129, 0.15);
    color: var(--color-status-done);
  }

  .status-badge.fail {
    background: rgba(248, 113, 113, 0.15);
    color: var(--color-status-fail);
  }

  .task-url {
    font-size: 12px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 6px;
  }

  .progress-bar {
    height: 6px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 3px;
    margin-bottom: 6px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--progress-gradient);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .progress-info {
    display: flex;
    gap: 12px;
    align-items: center;
    font-size: 12px;
    margin-bottom: 4px;
  }

  .progress-pct {
    color: var(--color-accent);
    font-weight: 600;
  }

  .speed {
    color: var(--color-text-secondary);
  }

  .threads {
    color: var(--color-text-secondary);
  }

  .error-msg {
    font-size: 12px;
    color: var(--color-status-fail);
    background: rgba(248, 113, 113, 0.08);
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    margin-bottom: 6px;
    word-break: break-all;
  }

  .output-path {
    font-size: 12px;
    color: var(--color-status-done);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 4px;
  }

  .task-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 6px;
  }

  .action-btn {
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
    padding: 0;
    line-height: 1;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--color-text-secondary);
    color: var(--color-text-main);
  }

  .action-btn.danger:hover {
    background: rgba(248, 113, 113, 0.1);
    border-color: var(--color-status-fail);
    color: var(--color-status-fail);
  }

  .action-btn.accent:hover {
    background: var(--color-accent-glow);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  /* Completed tasks are dimmed */
  .task-card.completed {
    opacity: 0.6;
  }

  .task-card.completed:hover {
    opacity: 0.8;
  }

  /* Log area */
  .log-area {
    margin-top: 10px;
    padding: 10px 12px;
    background: var(--color-bg-main);
    border-radius: var(--radius-sm);
    max-height: 180px;
    overflow-y: auto;
    font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
    font-size: 11.5px;
    line-height: 1.6;
  }

  .log-line {
    color: var(--color-text-secondary);
    white-space: pre-wrap;
    word-break: break-all;
  }

  .log-empty {
    color: var(--color-text-disabled);
    font-style: italic;
  }
</style>
