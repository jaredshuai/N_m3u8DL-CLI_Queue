<script>
  import { onMount } from 'svelte';
  import { dndzone } from 'svelte-dnd-action';
  import InputBar from './lib/InputBar.svelte';
  import CliConsolePanel from './lib/CliConsolePanel.svelte';
  import SettingsPanel from './lib/SettingsPanel.svelte';
  import TaskCard from './lib/TaskCard.svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import { getTaskIdSignature, toDndItems } from './lib/waiting-dnd.js';
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
    appSettings,
    cancelAutoShutdown,
    clearAppNotice,
    loadAppSettings,
    loadHistoryPage,
    loadInitialHistory,
    loadQueueState,
    setupListeners,
    shutdownNotice,
    tasks,
    teardownListeners,
  } from './lib/stores.js';
  import { invoke } from '@tauri-apps/api/core';

  let waitingTasks = $derived($tasks.filter(t => t.status === 'waiting'));
  let activeTasks = $derived($tasks.filter(t => t.status === 'downloading'));
  let cancelledTasks = $derived($tasks.filter(t => t.status === 'cancelled'));
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
  let waitingContentSignature = $derived(
    waitingTasks.map(t => `${t.id}\x00${t.saveName ?? ''}`).join('|')
  );
  let dndItemSignature = $derived(getTaskIdSignature(dndItems));
  let dndContentSignature = $derived(
    dndItems.map(t => `${t.id}\x00${t.saveName ?? ''}`).join('|')
  );

  const dndOptions = {
    flipDurationMs: 150,
    dragDisabled: false,
    dropFromOthersDisabled: true,
    centreDraggedOnCursor: true,
  };

  function applyThemePreference(preference = 'auto') {
    if (typeof document === 'undefined') return;
    const normalized = ['auto', 'dark', 'light'].includes(preference) ? preference : 'auto';
    // Software B: auto = no attribute (CSS media query resolves), dark/light = explicit
    if (normalized === 'auto') {
      document.documentElement.removeAttribute('data-theme');
    } else {
      document.documentElement.setAttribute('data-theme', normalized);
    }
  }

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
    if (dndSyncLocked) return;
    if (waitingTaskSignature === dndItemSignature &&
        waitingContentSignature === dndContentSignature) return;

    dndItems = toDndItems(waitingTasks);
  });

  $effect(() => {
    if (cliConsole.open && !cliConsoleTask) {
      cliConsole = closeCliConsole(cliConsole);
    }
  });

  // Theme switching: auto / dark / light
  $effect(() => {
    applyThemePreference($appSettings.theme);
  });

  // When following system, react to OS theme changes
  onMount(() => {
    const mediaQuery = window.matchMedia?.('(prefers-color-scheme: light)');
    if (!mediaQuery) return;

    const handleChange = () => {
      if (($appSettings.theme ?? 'auto') === 'auto') {
        applyThemePreference('auto');
      }
    };

    mediaQuery.addEventListener?.('change', handleChange);
    return () => mediaQuery.removeEventListener?.('change', handleChange);
  });
</script>

<main class="app">
  <div class="aurora" aria-hidden="true"></div>
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

        {#if cancelledTasks.length > 0}
          <div class="section-label cancelled-label">已停止</div>
          {#each cancelledTasks as task (task.id)}
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
          <div class="empty-icon">
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
              <polyline points="7 10 12 15 17 10"/>
              <line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
          </div>
          <p class="empty-title">粘贴 m3u8 链接即可开始</p>
          <p class="empty-hint">Ctrl+V 粘贴 · 回车添加</p>
        </div>
      {/if}
    </section>

    {#if cliConsole.open && cliConsoleTask}
      <div class="cli-console-overlay">
        <CliConsolePanel
          task={cliConsoleTask}
          onClose={handleCloseCliConsole}
          overlay={true}
        />
      </div>
    {/if}
  </section>

  <StatusBar />
</main>

<style>
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    color: var(--color-text-main);
    overflow: hidden;
  }

  .aurora {
    position: absolute;
    inset: -80px;
    z-index: 0;
    pointer-events: none;
    opacity: var(--aurora-opacity, 1);
    will-change: transform;
    background:
      radial-gradient(ellipse 65% 55% at 18% 8%, rgba(234, 179, 8, 0.45) 0%, transparent 60%),
      radial-gradient(ellipse 55% 50% at 82% 55%, rgba(217, 119, 6, 0.30) 0%, transparent 55%),
      radial-gradient(ellipse 60% 45% at 45% 95%, rgba(59, 130, 246, 0.22) 0%, transparent 50%);
    filter: blur(40px);
    animation: auroraShift 25s ease-in-out infinite alternate;
  }

  @keyframes auroraShift {
    0% {
      transform: translate(0, 0) scale(1);
    }
    33% {
      transform: translate(30px, -15px) scale(1.03);
    }
    66% {
      transform: translate(-20px, 12px) scale(0.97);
    }
    100% {
      transform: translate(12px, -8px) scale(1.01);
    }
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
    background: var(--scrim);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--color-border);
    box-shadow: 0 1px 8px rgba(234, 179, 8, 0.04), 0 4px 24px rgba(0, 0, 0, 0.15);
    padding-bottom: 14px;
    z-index: 10;
  }

  .shutdown-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-shrink: 0;
    padding: 10px 16px;
    border-bottom: 1px solid var(--color-accent-border);
    background: var(--color-warning-bg);
  }

  .shutdown-banner.error {
    border-bottom-color: var(--color-error-border);
    background: var(--color-error-bg);
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
    border: 1px solid var(--color-accent-border);
    border-radius: var(--radius-sm);
    background: var(--color-accent-soft-bg);
    color: var(--color-accent-bright);
    font-family: var(--font-stack);
    font-weight: 700;
    cursor: pointer;
  }

  .shutdown-banner button:hover {
    background: var(--color-accent-glow);
  }

  .app-notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-shrink: 0;
    padding: 10px 16px;
    border-bottom: 1px solid var(--color-error-border);
    background: var(--color-error-bg);
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
    border: 1px solid var(--color-error-border);
    border-radius: var(--radius-sm);
    background: var(--color-fail-soft-bg);
    color: var(--color-status-fail);
    font-family: var(--font-stack);
    font-weight: 700;
    cursor: pointer;
  }

  .app-notice button:hover {
    background: var(--color-error-bg);
  }

  .task-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px 18px;
  }

  .cli-console-overlay {
    position: absolute;
    inset: 0;
    z-index: 40;
    padding: 14px 16px 16px;
    background: var(--color-bg-terminal-overlay);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }

  .section-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1.2px;
    color: var(--color-text-disabled);
    margin-top: 20px;
    margin-bottom: 10px;
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
    gap: 8px;
  }

  .empty-icon {
    width: 56px;
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 16px;
    background: var(--color-accent-glow);
    color: var(--color-accent-dim);
    margin-bottom: 8px;
  }

  .empty-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text-secondary);
  }

  .empty-hint {
    font-size: 12px;
    color: var(--color-text-disabled);
    letter-spacing: 0.3px;
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
