<script>
  import { onMount } from 'svelte';
  import { dndzone } from 'svelte-dnd-action';
  import InputBar from './lib/InputBar.svelte';
  import CliConsolePanel from './lib/CliConsolePanel.svelte';
  import SettingsPanel from './lib/SettingsPanel.svelte';
  import TaskCard from './lib/TaskCard.svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import { getTaskIdSignature, shouldSyncDndItems, toDndItems } from './lib/waiting-dnd.js';
  import {
    closeCliConsole,
    createCliConsoleState,
    findCliConsoleTask,
    openCliConsole,
  } from './lib/cli-console.js';
  import {
    completedHistory,
    failedHistory,
    appNotice,
    cancelAutoShutdown,
    clearAppNotice,
    loadAppSettings,
    loadHistoryPage,
    loadInitialHistory,
    loadQueueState,
    setupListeners,
    shutdownNotice,
    tasks,
    terminalActiveLines,
    terminalCommittedLines,
    teardownListeners,
  } from './lib/stores.js';
  import { invoke } from '@tauri-apps/api/core';

  let waitingTasks = $derived($tasks.filter(t => t.status === 'waiting'));
  let activeTasks = $derived($tasks.filter(t => t.status === 'downloading'));
  let completedTasks = $derived($completedHistory.tasks);
  let failedTasks = $derived($failedHistory.tasks);
  let completedHasMore = $derived($completedHistory.hasMore);
  let failedHasMore = $derived($failedHistory.hasMore);
  let showSettings = $state(false);
  let dndItems = $state([]);
  let dndSyncLocked = $state(false);
  let historyLoading = $state({
    completed: false,
    failed: false,
  });
  let cliConsole = $state(createCliConsoleState());
  let waitingTaskSignature = $derived(getTaskIdSignature(waitingTasks));
  let dndItemSignature = $derived(getTaskIdSignature(dndItems));

  const dndOptions = {
    flipDurationMs: 150,
    dragDisabled: false,
    dropFromOthersDisabled: true,
    centreDraggedOnCursor: true,
  };

  function toggleSettings() {
    showSettings = !showSettings;
  }

  async function handleCancelShutdown() {
    await cancelAutoShutdown();
  }

  function handleOpenCliConsole(task) {
    cliConsole = openCliConsole(cliConsole, task.id);
  }

  function handleCloseCliConsole() {
    cliConsole = closeCliConsole(cliConsole);
  }

  function handleDndConsider(e) {
    dndSyncLocked = true;
    dndItems = e.detail.items;
  }

  async function handleDndFinalize(e) {
    dndSyncLocked = true;
    dndItems = e.detail.items;
    const newOrder = dndItems.map(item => item.id);
    try {
      await invoke('reorder_tasks', { taskIds: newOrder });
      await loadQueueState();
    } catch (err) {
      console.error('Failed to reorder tasks:', err);
      await loadQueueState();
    } finally {
      dndSyncLocked = false;
    }
  }

  let hasVisibleItems = $derived(
    $tasks.length > 0 || completedTasks.length > 0 || failedTasks.length > 0
  );
  let cliConsoleTask = $derived(findCliConsoleTask(cliConsole, {
    tasks: $tasks,
    completedTasks,
    failedTasks,
  }));
  let cliConsoleHasLiveActiveLine = $derived(
    cliConsole.taskId
      ? Object.prototype.hasOwnProperty.call($terminalActiveLines, cliConsole.taskId)
      : false
  );
  let cliConsoleLiveActiveLine = $derived(
    cliConsoleHasLiveActiveLine ? $terminalActiveLines[cliConsole.taskId] : ''
  );
  let cliConsoleLiveCommittedLines = $derived(
    cliConsole.taskId ? ($terminalCommittedLines[cliConsole.taskId] ?? []) : []
  );

  async function handleLoadMore(status) {
    if (historyLoading[status]) return;

    historyLoading = {
      ...historyLoading,
      [status]: true,
    };

    try {
      await loadHistoryPage(status);
    } finally {
      historyLoading = {
        ...historyLoading,
        [status]: false,
      };
    }
  }

  onMount(() => {
    let cancelled = false;

    async function initialize() {
      await loadQueueState();
      if (cancelled) return;

      await loadAppSettings();
      if (cancelled) return;

      await setupListeners();
      if (cancelled) {
        teardownListeners();
        return;
      }

      await loadInitialHistory();
    }

    initialize().catch((err) => {
      console.error('Failed to initialize app:', err);
      teardownListeners();
    });

    return () => {
      cancelled = true;
      teardownListeners();
    };
  });

  $effect(() => {
    const _waitingSignature = waitingTaskSignature;
    const _dndSignature = dndItemSignature;
    if (!shouldSyncDndItems({
      waitingTasks,
      dndItems,
      syncLocked: dndSyncLocked,
    })) {
      return;
    }

    dndItems = toDndItems(waitingTasks);
  });

  $effect(() => {
    if (cliConsole.open && !cliConsoleTask) {
      cliConsole = closeCliConsole(cliConsole);
    }
  });
</script>

<main class="app">
  <TitleBar onToggleSettings={toggleSettings} settingsOpen={showSettings} />

  <section class="app-shell">
    {#if showSettings}
      <SettingsPanel />
    {/if}

    <header class="app-header">
      <InputBar />
    </header>

    {#if $shutdownNotice.active || $shutdownNotice.error}
      <section class:error={$shutdownNotice.error} class="shutdown-banner" role="alert">
        {#if $shutdownNotice.active}
          <div>
            <strong>系统倒计时</strong>
            <span>队列已全部完成，系统操作将在 {$shutdownNotice.secondsRemaining} 秒后执行。</span>
          </div>
          <button onclick={handleCancelShutdown}>取消</button>
        {:else}
          <div>
            <strong>系统操作失败</strong>
            <span>{$shutdownNotice.error}</span>
          </div>
        {/if}
      </section>
    {/if}

    {#if $appNotice.message}
      <section class="app-notice error" role="alert">
        <div>
          <strong>{$appNotice.title}</strong>
          <span>{$appNotice.message}</span>
        </div>
        <button onclick={clearAppNotice}>关闭</button>
      </section>
    {/if}

    <section class="task-list" aria-hidden={cliConsole.open && cliConsoleTask ? 'true' : undefined}>
      {#if hasVisibleItems}
        {#if activeTasks.length > 0}
          <div class="section-label">下载中</div>
          {#each activeTasks as task (task.id)}
            <div class="fade-in">
              <TaskCard
                {task}
                draggable={false}
                onOpenCliConsole={handleOpenCliConsole}
                cliConsoleActive={cliConsole.open && cliConsole.taskId === task.id}
              />
            </div>
          {/each}
        {/if}

        {#if waitingTasks.length > 0}
          <div class="section-label">等待中</div>
          <div
            class="dnd-zone"
            use:dndzone={{ items: dndItems, ...dndOptions }}
            onconsider={handleDndConsider}
            onfinalize={handleDndFinalize}
          >
            {#each dndItems as task (task.id)}
              <div class="dnd-item">
                <TaskCard
                  {task}
                  draggable={true}
                  onOpenCliConsole={handleOpenCliConsole}
                  cliConsoleActive={cliConsole.open && cliConsole.taskId === task.id}
                />
              </div>
            {/each}
          </div>
        {/if}

        {#if failedTasks.length > 0}
          <div class="section-label">失败</div>
          {#each failedTasks as task (task.id)}
            <div class="fade-in">
              <TaskCard
                {task}
                draggable={false}
                historical={true}
                onOpenCliConsole={handleOpenCliConsole}
                cliConsoleActive={cliConsole.open && cliConsole.taskId === task.id}
              />
            </div>
          {/each}
        {#if failedHasMore}
            <button
              class="load-more-btn"
              onclick={() => handleLoadMore('failed')}
              disabled={historyLoading.failed}
            >
              {historyLoading.failed ? '加载中...' : '加载更多失败记录'}
            </button>
          {/if}
        {/if}

        {#if completedTasks.length > 0}
          <div class="section-label">已完成</div>
          {#each completedTasks as task (task.id)}
            <div class="fade-in">
              <TaskCard
                {task}
                draggable={false}
                historical={true}
                onOpenCliConsole={handleOpenCliConsole}
                cliConsoleActive={cliConsole.open && cliConsole.taskId === task.id}
              />
            </div>
          {/each}
          {#if completedHasMore}
            <button
              class="load-more-btn"
              onclick={() => handleLoadMore('completed')}
              disabled={historyLoading.completed}
            >
              {historyLoading.completed ? '加载中...' : '加载更多已完成记录'}
            </button>
          {/if}
        {/if}
      {:else}
        <div class="empty-state">
          <div class="empty-icon">📋</div>
          <p>队列为空，粘贴 m3u8 链接即可开始下载</p>
        </div>
      {/if}
    </section>

    {#if cliConsole.open && cliConsoleTask}
      <div class="cli-console-overlay">
        <CliConsolePanel
          task={cliConsoleTask}
          onClose={handleCloseCliConsole}
          overlay={true}
          liveCommittedLines={cliConsoleLiveCommittedLines}
          liveActiveLine={cliConsoleLiveActiveLine}
          hasLiveActiveLine={cliConsoleHasLiveActiveLine}
        />
      </div>
    {/if}
  </section>

  <StatusBar />
</main>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg-main);
    color: var(--color-text-main);
    overflow: hidden;
  }

  .app-shell {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .app-header {
    flex-shrink: 0;
    background: rgba(11, 13, 18, 0.85);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--color-border);
    padding-bottom: 12px;
    z-index: 10;
  }

  .shutdown-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-shrink: 0;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(234, 179, 8, 0.35);
    background: rgba(234, 179, 8, 0.11);
  }

  .shutdown-banner.error {
    border-bottom-color: rgba(248, 113, 113, 0.35);
    background: rgba(248, 113, 113, 0.08);
  }

  .shutdown-banner div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .shutdown-banner strong {
    color: var(--color-accent-bright);
    font-size: 13px;
  }

  .shutdown-banner.error strong {
    color: var(--color-status-fail);
  }

  .shutdown-banner span {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .shutdown-banner button {
    flex-shrink: 0;
    padding: 7px 12px;
    border: 1px solid rgba(234, 179, 8, 0.45);
    border-radius: var(--radius-sm);
    background: rgba(234, 179, 8, 0.12);
    color: var(--color-accent-bright);
    font-family: var(--font-stack);
    font-weight: 700;
    cursor: pointer;
  }

  .shutdown-banner button:hover {
    background: rgba(234, 179, 8, 0.18);
  }

  .app-notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-shrink: 0;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(248, 113, 113, 0.35);
    background: rgba(248, 113, 113, 0.08);
  }

  .app-notice div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .app-notice strong {
    color: var(--color-status-fail);
    font-size: 13px;
  }

  .app-notice span {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .app-notice button {
    flex-shrink: 0;
    padding: 7px 12px;
    border: 1px solid rgba(248, 113, 113, 0.45);
    border-radius: var(--radius-sm);
    background: rgba(248, 113, 113, 0.12);
    color: var(--color-status-fail);
    font-family: var(--font-stack);
    font-weight: 700;
    cursor: pointer;
  }

  .app-notice button:hover {
    background: rgba(248, 113, 113, 0.18);
  }

  .task-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 16px;
  }

  .cli-console-overlay {
    position: absolute;
    inset: 0;
    z-index: 40;
    padding: 14px 16px 16px;
    background:
      linear-gradient(180deg, rgba(7, 9, 13, 0.82), rgba(4, 6, 10, 0.9)),
      radial-gradient(circle at top right, rgba(250, 204, 21, 0.06), transparent 30%);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }

  .section-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--color-text-disabled);
    margin-top: 16px;
    margin-bottom: 8px;
    padding-left: 2px;
  }

  .section-label:first-child {
    margin-top: 0;
  }

  .dnd-zone {
    display: flex;
    flex-direction: column;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 12px;
  }

  .empty-icon {
    font-size: 48px;
    opacity: 0.3;
  }

  .empty-state p {
    font-size: 14px;
    color: var(--color-text-disabled);
  }

  .load-more-btn {
    display: block;
    width: 100%;
    margin-top: 8px;
    padding: 10px 12px;
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-family: var(--font-stack);
    cursor: pointer;
    transition: border-color 0.2s, color 0.2s, background 0.2s;
  }

  .load-more-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-glow);
  }

  .load-more-btn:disabled {
    opacity: 0.5;
    cursor: progress;
  }

  @media (max-width: 640px) {
    .cli-console-overlay {
      padding: 10px 10px 12px;
    }
  }
</style>
