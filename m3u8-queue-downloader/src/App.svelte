<script>
  import { onMount } from 'svelte';
  import { dndzone } from 'svelte-dnd-action';
  import InputBar from './lib/InputBar.svelte';
  import SettingsPanel from './lib/SettingsPanel.svelte';
  import TaskCard from './lib/TaskCard.svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import {
    completedHistory,
    failedHistory,
    cancelAutoShutdown,
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
  let completedTasks = $derived($completedHistory.tasks);
  let failedTasks = $derived($failedHistory.tasks);
  let completedHasMore = $derived($completedHistory.hasMore);
  let failedHasMore = $derived($failedHistory.hasMore);
  let showSettings = $state(false);
  let dndItems = $state([]);

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

  function handleDndConsider(e) {
    dndItems = e.detail.items;
  }

  async function handleDndFinalize(e) {
    dndItems = e.detail.items;
    const newOrder = dndItems.map(item => item.id);
    try {
      await invoke('reorder_tasks', { taskIds: newOrder });
      await loadQueueState();
    } catch (err) {
      console.error('Failed to reorder tasks:', err);
      await loadQueueState();
    }
  }

  let hasVisibleItems = $derived(
    $tasks.length > 0 || completedTasks.length > 0 || failedTasks.length > 0
  );

  async function handleLoadMore(status) {
    await loadHistoryPage(status);
  }

  onMount(async () => {
    await loadQueueState();
    await loadAppSettings();
    await setupListeners();
    await loadInitialHistory();

    return () => {
      teardownListeners();
    };
  });

  $effect(() => {
    dndItems = waitingTasks.map((task) => ({ ...task, id: task.id }));
  });
</script>

<main class="app">
  <TitleBar onToggleSettings={toggleSettings} settingsOpen={showSettings} />

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

  <section class="task-list">
    {#if hasVisibleItems}
      {#if activeTasks.length > 0}
        <div class="section-label">下载中</div>
        {#each activeTasks as task (task.id)}
          <div class="fade-in">
            <TaskCard {task} draggable={false} />
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
              <TaskCard {task} draggable={true} />
            </div>
          {/each}
        </div>
      {/if}

      {#if failedTasks.length > 0}
        <div class="section-label">失败</div>
        {#each failedTasks as task (task.id)}
          <div class="fade-in">
            <TaskCard {task} draggable={false} historical={true} />
          </div>
        {/each}
        {#if failedHasMore}
          <button class="load-more-btn" onclick={() => handleLoadMore('failed')}>
            加载更多失败记录
          </button>
        {/if}
      {/if}

      {#if completedTasks.length > 0}
        <div class="section-label">已完成</div>
        {#each completedTasks as task (task.id)}
          <div class="fade-in">
            <TaskCard {task} draggable={false} historical={true} />
          </div>
        {/each}
        {#if completedHasMore}
          <button class="load-more-btn" onclick={() => handleLoadMore('completed')}>
            加载更多已完成记录
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

  .task-list {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
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
</style>
