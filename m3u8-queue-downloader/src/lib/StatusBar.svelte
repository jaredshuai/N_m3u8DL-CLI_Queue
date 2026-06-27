<script>
  import { failedHistory, queueRunning, sessionCompletedCount, tasks } from './stores.js';

  let waitingCount = $derived(
    $tasks.filter(t => t.status === 'waiting').length
  );
  let downloadingCount = $derived(
    $tasks.filter(t => t.status === 'downloading').length
  );
  let failedCount = $derived($failedHistory.tasks.length);
  let isDraining = $derived(!$queueRunning && downloadingCount > 0);

  let currentLabel = $derived(
    downloadingCount > 0
      ? isDraining
        ? `下载中 (${downloadingCount})，后续已暂停`
        : `下载中 (${downloadingCount})`
      : '空闲'
  );

  // Live speed of the currently downloading task (only one runs at a time).
  // Empty when no download is active or before the first progress event arrives.
  let currentSpeed = $derived(
    $tasks.find(t => t.status === 'downloading')?.speed ?? ''
  );
</script>

<div class="status-bar">
  <div class="status-info">
    <span class="status-item">
      <span class="dot wait"></span>
      队列: {waitingCount} 等待中
    </span>
    <span class="status-divider">|</span>
    <span class="status-item">
      <span class="dot down"></span>
      当前: {currentLabel}
    </span>
    <span class="status-divider">|</span>
    <span class="status-item">
      <span class="dot done"></span>
      已完成: {$sessionCompletedCount}
    </span>
    <span class="status-divider">|</span>
    <span class="status-item">
      <span class="dot fail"></span>
      失败: {failedCount}
    </span>
  </div>
  {#if currentSpeed}
    <div class="status-meta">
      <span class="dot down"></span>
      {currentSpeed}
    </div>
  {/if}
</div>

<style>
  .status-bar {
    position: relative;
    z-index: 1;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 18px;
    border-top: 1px solid var(--color-border);
    box-shadow: 0 -1px 4px rgba(234, 179, 8, 0.03);
    background: var(--color-bg-card-alpha);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
  }

  .status-info {
    display: flex;
    gap: 4px;
    align-items: center;
    flex-wrap: wrap;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--color-text-secondary);
  }

  .status-divider {
    color: var(--color-text-disabled);
    font-size: 11px;
    margin: 0 4px;
  }

  .status-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot.wait {
    background: var(--color-status-wait);
  }

  .dot.down {
    background: var(--color-status-down);
    animation: status-pulse 2s ease-in-out infinite;
  }

  .dot.done {
    background: var(--color-status-done);
  }

  .dot.fail {
    background: var(--color-status-fail);
  }
</style>
