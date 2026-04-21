<script>
  import { onMount } from 'svelte';
  import { dndzone } from 'svelte-dnd-action';
  import InputBar from './lib/InputBar.svelte';
  import TaskCard from './lib/TaskCard.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import {
    completedHistory,
    failedHistory,
    loadHistoryPage,
    loadInitialHistory,
    loadQueueState,
    setupListeners,
    tasks,
    teardownListeners,
  } from './lib/stores.js';
  import { invoke } from '@tauri-apps/api/core';

  // DND: only Waiting tasks are draggable
  // We split tasks into waiting list (dnd-enabled) and others (static)
  let waitingTasks = $derived($tasks.filter(t => t.status === 'waiting'));
  let activeTasks = $derived($tasks.filter(t => t.status === 'downloading'));
  let completedTasks = $derived($completedHistory.tasks);
  let failedTasks = $derived($failedHistory.tasks);
  let completedHasMore = $derived($completedHistory.hasMore);
  let failedHasMore = $derived($failedHistory.hasMore);

  // DND state - local copy of waiting items for the dnd zone
  let dndItems = $state([]);

  // DND options
  const dndOptions = {
    flipDurationMs: 150,
    dragDisabled: false,
    dropFromOthersDisabled: false,
    centreDraggedOnCursor: true,
  };

  function handleDndConsider(e) {
    // Items are being dragged within the zone - update local order
    dndItems = e.detail.items;
  }

  async function handleDndFinalize(e) {
    dndItems = e.detail.items;
    // Extract new order of task IDs and notify backend
    const newOrder = dndItems.map(item => item.id);
    try {
      await invoke('reorder_tasks', { taskIds: newOrder });
      await loadQueueState();
    } catch (err) {
      console.error('Failed to reorder tasks:', err);
      // Reload to revert visual order
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
  <!-- Fixed header with blur -->
  <header class="app-header">
    <div class="header-title">
      <span class="header-icon">⬇</span>
      <h1>m3u8 Queue Downloader</h1>
    </div>
    <InputBar />
  </header>

  <!-- Scrollable task list -->
  <section class="task-list">
    {#if hasVisibleItems}
      <!-- Active (downloading) tasks -->
      {#if activeTasks.length > 0}
        <div class="section-label">下载中</div>
        {#each activeTasks as task (task.id)}
          <div class="fade-in">
            <TaskCard {task} draggable={false} />
          </div>
        {/each}
      {/if}

      <!-- Waiting tasks with drag-and-drop -->
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

      <!-- Failed tasks -->
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

      <!-- Completed tasks -->
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

  <!-- Bottom status bar -->
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
    padding: 12px 16px 12px;
    z-index: 10;
  }

  .header-title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .header-icon {
    font-size: 18px;
    color: var(--color-accent);
  }

  .header-title h1 {
    font-size: 16px;
    font-weight: 700;
    color: var(--color-text-main);
    margin: 0;
    letter-spacing: -0.3px;
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

  .dnd-item {
    /* DnD wrapper - no extra spacing, TaskCard handles its own margin */
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
