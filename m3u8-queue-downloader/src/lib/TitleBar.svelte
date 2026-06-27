<script>
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { appSettings, saveAppSettings } from './stores.js';

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

  async function cycleTheme() {
    const current = $appSettings.theme ?? 'auto';
    const next = current === 'auto' ? 'dark' : current === 'dark' ? 'light' : 'auto';
    try {
      await saveAppSettings({ ...$appSettings, theme: next });
    } catch (err) {
      console.error('Failed to change theme:', err);
    }
  }

  let themeLabel = $derived(
    ($appSettings.theme ?? 'auto') === 'auto' ? '跟随系统' :
    ($appSettings.theme ?? 'auto') === 'dark' ? '深色' : '浅色'
  );

  let themeIcon = $derived(
    ($appSettings.theme ?? 'auto') === 'auto' ? '◐' :
    ($appSettings.theme ?? 'auto') === 'dark' ? '☾' : '☀'
  );
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
      class="title-btn theme-btn"
      onclick={cycleTheme}
      title={`主题：${themeLabel}（点击切换）`}
      aria-label={`主题：${themeLabel}`}
    >
      {themeIcon}
    </button>
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
    background: var(--color-bg-titlebar-overlay);
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
    box-shadow: inset 0 1px 0 var(--overlay-active);
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
    background: var(--overlay-hover);
    border-color: var(--color-border);
    color: var(--color-accent-bright);
  }

  .settings-btn {
    margin-right: 4px;
    font-size: 13px;
  }

  .title-btn.close:hover {
    background: var(--color-fail-soft-bg);
    border-color: var(--color-error-border);
    color: var(--color-status-fail);
  }
</style>
