<script>
  import { invoke } from '@tauri-apps/api/core';
  import { displayProgressPercent } from './progress.js';
  import { loadQueueState, stopTask } from './queue-store.js';
  import { appSettings, clearHistoryTask, trackSessionTask } from './stores.js';

  let { task, draggable = false, historical = false, onOpenCliConsole = null, cliConsoleActive = false } = $props();

  let statusKey = $derived(
    task.status === 'downloading' ? 'down' :
    task.status === 'waiting' ? 'wait' :
    task.status === 'completed' ? 'done' :
    task.status === 'failed' ? 'fail' :
    task.status === 'cancelled' ? 'cancel' : 'wait'
  );

  let statusLabel = $derived(
    task.status === 'downloading' ? '下载中' :
    task.status === 'waiting' ? '等待中' :
    task.status === 'completed' ? '已完成' :
    task.status === 'failed' ? '失败' :
    task.status === 'cancelled' ? '已停止' : task.status
  );

  let displayTitle = $derived(
    task.saveName || (task.status === 'waiting' ? '自动识别中...（点击改名）' : task.url)
  );

  let progressPct = $derived(displayProgressPercent(task.progress));
  let canShowCliLive = $derived(statusKey === 'down' || statusKey === 'done' || statusKey === 'fail' || statusKey === 'cancel');

  let editing = $state(false);
  let draftName = $state('');
  let committing = $state(false);
  let stopping = $state(false);

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

  async function handleStop() {
    if (stopping) return;
    stopping = true;
    try {
      await stopTask(task.id);
    } catch (err) {
      console.error('Failed to stop task:', err);
    } finally {
      stopping = false;
    }
  }

  async function handleClearHistory() {
    const status = task.status === 'failed' ? 'failed' : 'completed';
    try {
      await clearHistoryTask(status, task.id);
    } catch (err) {
      console.error('Failed to clear history task:', err);
    }
  }

  function handleOpenCliConsole() {
    onOpenCliConsole?.(task);
  }

  function startEdit() {
    draftName = task.saveName ?? '';
    editing = true;
  }

  function cancelEdit() {
    if (committing) return;
    editing = false;
  }

  async function commitEdit() {
    if (!editing || committing) return;
    committing = true;
    const trimmed = draftName.trim();
    const current = task.saveName ?? '';
    const unchanged = trimmed === current;
    editing = false;
    if (unchanged) {
      committing = false;
      return;
    }
    try {
      await invoke('update_save_name', {
        taskId: task.id,
        saveName: trimmed === '' ? null : trimmed,
      });
      await loadQueueState();
    } catch (err) {
      console.error('Failed to update save name:', err);
    } finally {
      committing = false;
    }
  }
</script>

<div
  class="task-card"
  class:downloading={statusKey === 'down'}
  class:completed={statusKey === 'done'}
>
  <div class="card-main">
    {#if draggable}
      <div class="drag-handle" title="拖动排序">⠿</div>
    {/if}

    <div class="card-content">
      <div class="title-row">
        {#if statusKey === 'wait' && editing}
          <div class="save-name-edit">
            <input
              class="save-name-input"
              bind:value={draftName}
              onkeydown={(e) => {
                if (e.key === 'Enter') commitEdit();
                else if (e.key === 'Escape') cancelEdit();
              }}
              onblur={commitEdit}
              placeholder="留空则自动识别"
              maxlength="120"
            />
            <button type="button" class="edit-confirm" onclick={commitEdit} title="保存">✓</button>
            <button type="button" class="edit-cancel" onclick={cancelEdit} title="取消">✕</button>
          </div>
        {:else}
          <button
            type="button"
            class="task-title"
            class:editable={statusKey === 'wait'}
            onclick={statusKey === 'wait' ? startEdit : null}
            disabled={statusKey !== 'wait'}
          >{displayTitle}</button>
        {/if}
        <span class="status-badge {statusKey}">{statusLabel}</span>
      </div>

      <div class="task-url" title={task.url}>{task.url}</div>

      {#if statusKey === 'down'}
        <div class="progress-bar" aria-label="下载进度 {progressPct}%">
          <div class="progress-fill" style="width: {progressPct}%"></div>
        </div>
        <div class="progress-info">
          <span class="progress-pct">{progressPct}%</span>
          {#if task.speed}
            <span class="speed">{task.speed}</span>
          {/if}
          {#if task.threads}
            <span class="threads">线程 {task.threads}</span>
          {/if}
        </div>
      {/if}

      {#if statusKey === 'fail' && task.errorMessage}
        <div class="error-msg">{task.errorMessage}</div>
      {/if}

      {#if statusKey === 'cancel' && task.errorMessage}
        <div class="error-msg cancel-msg">{task.errorMessage}</div>
      {/if}

      {#if statusKey === 'done'}
        {#if task.outputPath}
          <div class="output-path" title={task.outputPath}>📁 {task.outputPath}</div>
        {:else if $appSettings.downloadDir}
          <div class="output-path" title={$appSettings.downloadDir}>📂 目录: {$appSettings.downloadDir}</div>
        {/if}
      {/if}

      <div class="task-actions">
        {#if canShowCliLive}
          <button class="action-btn text" onclick={handleOpenCliConsole} title="打开 CLI 终端面板">
            {cliConsoleActive ? '正在查看 CLI 终端' : '打开 CLI 终端'}
          </button>
        {/if}
        {#if statusKey === 'down'}
          <button class="action-btn danger" onclick={handleStop} disabled={stopping} title="停止">
            {stopping ? '停止中...' : '⏹'}
          </button>
        {/if}
        {#if statusKey === 'wait'}
          <button class="action-btn danger" onclick={handleRemove} title="删除">✕</button>
        {:else if statusKey === 'fail' || statusKey === 'cancel'}
          <button class="action-btn accent" onclick={handleRetry} title="重试">🔄</button>
          <button
            class="action-btn danger"
            onclick={historical ? handleClearHistory : handleRemove}
            title={historical ? '清除记录' : '删除'}
          >
            ✕
          </button>
        {:else if historical}
          <button class="action-btn danger" onclick={handleClearHistory} title="清除记录">✕</button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .task-card {
    position: relative;
    background: var(--color-bg-card-alpha);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 16px 18px;
    margin-bottom: 10px;
    box-shadow: var(--shadow-card);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    transition: background 0.15s, box-shadow 0.2s ease;
  }

  /* Card-level aurora: cool teal glow from bottom-left, contrasting warm background aurora */
  .task-card::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    opacity: var(--card-aurora-opacity, 1);
    background: radial-gradient(ellipse 90% 65% at 10% 105%, rgba(20, 184, 166, 0.28) 0%, transparent 55%);
    pointer-events: none;
    transition: opacity 0.3s ease;
  }

  .task-card:hover {
    background: var(--color-bg-card-hover-alpha);
    box-shadow: var(--shadow-card-hover);
  }

  .task-card:hover::before {
    opacity: 0.85;
  }

  .task-card.downloading {
    border-color: var(--color-status-down);
    box-shadow: 0 0 0 1px var(--color-status-down-bg), var(--shadow-card);
  }

  .task-card.downloading::before {
    background: radial-gradient(ellipse 95% 70% at 10% 105%, rgba(139, 92, 246, 0.38) 0%, transparent 50%);
    opacity: var(--card-aurora-opacity, 1);
  }

  .task-card.downloading:hover {
    box-shadow: 0 0 0 1px var(--color-status-down-bg), var(--shadow-card-hover);
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
    margin-bottom: 6px;
  }

  .task-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--color-text-main);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    /* button reset */
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font-family: inherit;
    text-align: left;
    cursor: default;
  }

  .task-title:disabled {
    cursor: default;
  }

  .task-title.editable {
    cursor: pointer;
  }

  .task-title.editable:hover {
    color: var(--color-accent);
  }

  .save-name-input {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--color-text-main);
    background: var(--color-bg-input);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius);
    padding: 2px 8px;
    min-width: 0;
    flex: 1;
    outline: none;
    font-family: inherit;
  }

  .save-name-edit {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .edit-confirm,
  .edit-cancel {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: 13px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    font-family: var(--font-stack);
    transition: all 0.15s;
  }

  .edit-confirm {
    background: var(--color-done-soft-bg);
    color: var(--color-status-done);
  }

  .edit-confirm:hover {
    background: var(--color-done-soft-bg);
    border-color: var(--color-status-done);
  }

  .edit-cancel {
    background: transparent;
    color: var(--color-text-secondary);
  }

  .edit-cancel:hover {
    color: var(--color-status-fail);
    border-color: var(--color-status-fail);
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
    background: var(--color-status-wait-bg);
    color: var(--color-status-wait);
  }

  .status-badge.down {
    background: var(--color-status-down-bg);
    color: var(--color-status-down);
    animation: status-pulse 2s ease-in-out infinite;
  }

  .status-badge.done {
    background: var(--color-status-done-bg);
    color: var(--color-status-done);
  }

  .status-badge.fail {
    background: var(--color-status-fail-bg);
    color: var(--color-status-fail);
  }

  .status-badge.cancel {
    background: var(--color-status-cancel-bg);
    color: var(--color-text-secondary);
  }

  .task-url {
    font-size: 12px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 8px;
  }

  .progress-bar {
    height: 4px;
    background: var(--progress-track);
    border-radius: 2px;
    margin-bottom: 8px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--progress-gradient);
    border-radius: 2px;
    transition: width 0.3s ease;
    position: relative;
    overflow: hidden;
  }

  .progress-fill::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    animation: shimmer 2s ease-in-out infinite;
  }

  .progress-info {
    display: flex;
    gap: 12px;
    align-items: center;
    font-size: 12px;
    margin-bottom: 8px;
  }

  .progress-pct {
    color: var(--color-accent);
    font-weight: 600;
  }

  .speed,
  .threads {
    color: var(--color-text-secondary);
  }

  .error-msg {
    font-size: 12px;
    color: var(--color-status-fail);
    background: var(--color-error-bg);
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    margin-bottom: 6px;
    word-break: break-all;
  }

  .error-msg.cancel-msg {
    color: var(--color-text-secondary);
    background: var(--overlay-hover);
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
    margin-top: 10px;
    flex-wrap: wrap;
  }

  .action-btn {
    min-width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
    padding: 0 8px;
    line-height: 1;
    font-family: var(--font-stack);
  }

  .action-btn.text {
    width: auto;
    font-size: 12px;
    font-weight: 700;
  }

  .action-btn:hover {
    background: var(--overlay-subtle);
    border-color: var(--color-text-secondary);
    color: var(--color-text-main);
  }

  .action-btn.danger:hover {
    background: var(--color-error-bg);
    border-color: var(--color-status-fail);
    color: var(--color-status-fail);
  }

  .action-btn.accent:hover {
    background: var(--color-accent-glow);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .task-card.completed {
    opacity: 0.72;
  }

  .task-card.completed:hover {
    opacity: 0.9;
  }
</style>
