<script>
  import { sessionCompletedCount, tasks } from './stores.js';

  let waitingCount = $derived(
    $tasks.filter(t => t.status === 'waiting').length
  );
  let downloadingCount = $derived(
    $tasks.filter(t => t.status === 'downloading').length
  );
  let failedCount = $derived(
    $tasks.filter(t => t.status === 'failed').length
  );

  let currentLabel = $derived(
    downloadingCount > 0 ? `下载中 (${downloadingCount})` : '空闲'
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
</div>

<style>
  .status-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-card);
    box-shadow: var(--card-inner-shadow, inset 0 1px 0 rgba(255,255,255,0.05));
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
    gap: 5px;
    font-size: 12px;
    color: var(--color-text-secondary);
  }

  .status-divider {
    color: var(--color-text-disabled);
    font-size: 11px;
    margin: 0 4px;
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
  }

  .dot.done {
    background: var(--color-status-done);
  }

  .dot.fail {
    background: var(--color-status-fail);
  }
</style>
