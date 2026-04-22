<script>
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { onToggleSettings, settingsOpen = false } = $props();

  async function startDragging(event) {
    if (event.button !== 0) return;

    try {
      await getCurrentWindow().startDragging();
    } catch (err) {
      console.error('Failed to start dragging window:', err);
    }
  }

  async function minimize() {
    try {
      await invoke('minimize_main_window');
    } catch (err) {
      console.error('Failed to minimize window:', err);
    }
  }

  async function toggleMaximize() {
    try {
      await invoke('toggle_main_window_maximize');
    } catch (err) {
      console.error('Failed to toggle maximize:', err);
    }
  }

  async function closeWindow() {
    try {
      await invoke('request_main_window_close');
    } catch (err) {
      console.error('Failed to close window:', err);
    }
  }
</script>

<header class="title-bar">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="title-area"
    data-tauri-drag-region
    onmousedown={startDragging}
  >
    <span class="app-mark">⬇</span>
    <div class="title-text" data-tauri-drag-region>
      <strong data-tauri-drag-region>m3u8 队列下载器</strong>
      <span data-tauri-drag-region>桌面队列工具</span>
    </div>
  </div>

  <div class="window-actions">
    <button
      class:active={settingsOpen}
      class="title-btn settings-btn"
      onclick={onToggleSettings}
      title="设置"
      aria-label="设置"
    >
      ⚙
    </button>
    <button class="title-btn" onclick={minimize} title="最小化" aria-label="最小化">—</button>
    <button class="title-btn" onclick={toggleMaximize} title="最大化/还原" aria-label="最大化/还原">□</button>
    <button class="title-btn close" onclick={closeWindow} title="关闭" aria-label="关闭">×</button>
  </div>
</header>

<style>
  .title-bar {
    height: 42px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    padding: 0 8px 0 14px;
    background: linear-gradient(180deg, rgba(19, 22, 28, 0.98), rgba(11, 13, 18, 0.96));
    border-bottom: 1px solid var(--color-border);
    user-select: none;
  }

  .title-area {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 1;
    min-width: 0;
    color: var(--color-text-main);
  }

  .app-mark {
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    background: var(--color-accent-glow);
    color: var(--color-accent-bright);
    font-size: 13px;
    box-shadow: inset 0 1px 0 rgba(255,255,255,0.08);
  }

  .title-text {
    display: flex;
    flex-direction: column;
    line-height: 1.1;
    min-width: 0;
  }

  .title-text strong {
    font-size: 12.5px;
    letter-spacing: 0.1px;
  }

  .title-text span {
    margin-top: 2px;
    font-size: 10.5px;
    color: var(--color-text-secondary);
  }

  .window-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .title-btn {
    width: 34px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font-family: var(--font-stack);
    font-size: 15px;
    line-height: 1;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .title-btn:hover,
  .title-btn.active {
    background: rgba(255,255,255,0.06);
    border-color: var(--color-border);
    color: var(--color-accent-bright);
  }

  .settings-btn {
    margin-right: 4px;
    font-size: 13px;
  }

  .title-btn.close:hover {
    background: rgba(248, 113, 113, 0.14);
    border-color: rgba(248, 113, 113, 0.38);
    color: var(--color-status-fail);
  }
</style>
