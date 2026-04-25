<script>
  import { invoke } from '@tauri-apps/api/core';
  import { loadQueueState, tasks, queueRunning, trackSessionTask } from './stores.js';
  import { getQueueControlState, runQueueToggle } from './queue-controls.js';
  import { findDuplicateWarnings } from './duplicate-warnings.js';

  let url = $state('');
  let saveName = $state('');
  let headers = $state('');
  let showAdvanced = $state(false);
  let showHeaders = $state(false);
  let adding = $state(false);
  let queueBusy = $state(false);
  let hasHeaders = $derived(headers.trim().length > 0);

  let duplicateWarnings = $derived(findDuplicateWarnings({
    tasks: $tasks,
    url,
    saveName,
  }));

  let queueControl = $derived(getQueueControlState({
    tasks: $tasks,
    queueRunning: $queueRunning,
    busy: queueBusy,
  }));

  async function handleAdd() {
    const trimmedUrl = url.trim();
    if (!trimmedUrl || adding) return;

    adding = true;
    try {
      const task = await invoke('add_task', {
        url: trimmedUrl,
        saveName: saveName.trim() || null,
        headers: headers.trim() || null,
      });
      trackSessionTask(task.id);
      url = '';
      saveName = '';
      await loadQueueState();
    } catch (err) {
      console.error('Failed to add task:', err);
    } finally {
      adding = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Enter' && !e.isComposing) {
      e.preventDefault();
      handleAdd();
    }
  }

  function toggleAdvanced() {
    showAdvanced = !showAdvanced;
  }

  function toggleHeaders() {
    showHeaders = !showHeaders;
  }

  function clearHeaders() {
    headers = '';
    showHeaders = false;
  }

  async function handleQueueToggle() {
    await runQueueToggle({
      disabled: queueControl.disabled,
      action: queueControl.action,
      setBusy: (value) => {
        queueBusy = value;
      },
      reloadQueueState: loadQueueState,
      onError: (err) => {
        console.error('Failed to toggle queue:', err);
      },
    });
  }
</script>

<div class="input-bar-wrapper">
  <div class="input-bar">
    <input
      type="text"
      bind:value={url}
      onkeydown={handleKeydown}
      placeholder="粘贴 m3u8 链接，回车添加到队列..."
      class="url-input"
      disabled={adding}
    />
    <button onclick={handleAdd} class="add-btn" disabled={!url.trim() || adding}>
      {adding ? '添加中...' : '添加'}
    </button>
    <button
      onclick={handleQueueToggle}
      class="queue-btn"
      disabled={queueControl.disabled}
    >
      {queueBusy ? '处理中...' : queueControl.label}
    </button>
  </div>

  {#if duplicateWarnings.length > 0}
    <div class="duplicate-warning" role="alert" aria-live="polite">
      <span class="warning-icon">⚠</span>
      <div class="warning-copy">
        <strong>可能重复</strong>
        <span>{duplicateWarnings.map((warning) => warning.message).join('；')}</span>
      </div>
    </div>
  {/if}

  <button class="advanced-toggle" onclick={toggleAdvanced}>
    {showAdvanced ? '▾ 高级选项' : '▸ 高级选项'}
  </button>

  {#if showAdvanced}
    <div class="advanced-panel fade-in">
      <div class="advanced-grid">
        <div class="field field-full">
          <label class="field-label" for="save-name">保存名称</label>
          <input
            id="save-name"
            type="text"
            bind:value={saveName}
            placeholder="可选，留空自动识别"
            class="field-input"
          />
        </div>

        <div class="headers-section">
          <div class="headers-row">
            <div class="headers-copy">
              <label class="field-label" for="headers-input">请求头</label>
              <span class="headers-hint">
                {#if hasHeaders}
                  当前已保留自定义请求头
                {:else}
                  通常无需填写，只有少数站点需要自定义 Referer / User-Agent
                {/if}
              </span>
            </div>
            <div class="headers-actions">
              {#if hasHeaders}
                <button class="mini-action danger" onclick={clearHeaders} type="button">清空</button>
              {/if}
              <button class="mini-action" onclick={toggleHeaders} type="button">
                {showHeaders ? '收起请求头' : '编辑请求头'}
              </button>
            </div>
          </div>

          {#if !showHeaders && hasHeaders}
            <div class="headers-preview" title={headers}>
              {headers}
            </div>
          {/if}

          {#if showHeaders}
            <div class="field field-full fade-in">
              <input
                id="headers-input"
                type="text"
                bind:value={headers}
                placeholder='如: "Referer:xxx" "User-Agent:xxx"'
                class="field-input"
              />
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .input-bar-wrapper {
    padding: 12px 16px 0;
  }

  .input-bar {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .url-input {
    flex: 1;
    padding: 10px 14px;
    background: var(--color-bg-input);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-main);
    font-size: 13.5px;
    font-family: var(--font-stack);
    outline: none;
    transition: border-color 0.2s;
  }

  .url-input::placeholder {
    color: var(--color-text-disabled);
  }

  .url-input:focus {
    border-color: var(--color-accent);
  }

  .url-input:disabled {
    opacity: 0.6;
  }

  .add-btn {
    padding: 10px 20px;
    background: var(--color-accent);
    color: var(--color-bg-main);
    border: none;
    border-radius: var(--radius-sm);
    font-weight: 700;
    font-size: 13.5px;
    font-family: var(--font-stack);
    cursor: pointer;
    transition: background 0.2s, transform 0.1s;
    white-space: nowrap;
  }

  .queue-btn {
    padding: 10px 20px;
    background: transparent;
    color: var(--color-text-main);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-weight: 700;
    font-size: 13.5px;
    font-family: var(--font-stack);
    cursor: pointer;
    transition: background 0.2s, transform 0.1s;
    white-space: nowrap;
  }

  .add-btn:hover:not(:disabled) {
    background: var(--color-accent-bright);
  }

  .queue-btn:hover:not(:disabled) {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-glow);
  }

  .add-btn:active:not(:disabled),
  .queue-btn:active:not(:disabled) {
    transform: scale(0.97);
  }

  .add-btn:disabled,
  .queue-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .advanced-toggle {
    display: block;
    margin-top: 6px;
    padding: 0;
    background: none;
    border: none;
    color: var(--color-text-secondary);
    font-size: 12px;
    font-family: var(--font-stack);
    cursor: pointer;
    transition: color 0.2s;
  }

  .advanced-toggle:hover {
    color: var(--color-accent);
  }

  .advanced-panel {
    margin-top: 8px;
    padding-bottom: 4px;
  }

  .advanced-grid {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-full {
    width: 100%;
  }

  .field-label {
    font-size: 11px;
    color: var(--color-text-secondary);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .field-input {
    padding: 8px 12px;
    background: var(--color-bg-input);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-main);
    font-size: 13px;
    font-family: var(--font-stack);
    outline: none;
    transition: border-color 0.2s;
  }

  .field-input::placeholder {
    color: var(--color-text-disabled);
  }

  .field-input:focus {
    border-color: var(--color-accent);
  }

  .headers-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.02);
  }

  .headers-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .headers-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .headers-hint {
    font-size: 12px;
    color: var(--color-text-disabled);
    line-height: 1.4;
  }

  .headers-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .mini-action {
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: var(--color-text-secondary);
    font-size: 12px;
    font-family: var(--font-stack);
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.2s, color 0.2s, background 0.2s;
  }

  .mini-action:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-glow);
  }

  .mini-action.danger:hover {
    border-color: rgba(248, 113, 113, 0.38);
    color: var(--color-status-fail);
    background: rgba(248, 113, 113, 0.1);
  }

  .headers-preview {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.18);
    color: var(--color-text-secondary);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .duplicate-warning {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 8px;
    padding: 8px 10px;
    border: 1px solid rgba(234, 179, 8, 0.45);
    border-radius: var(--radius-sm);
    background: rgba(234, 179, 8, 0.1);
    color: var(--color-accent-bright);
    font-size: 12px;
  }

  .warning-icon {
    line-height: 1.5;
  }

  .warning-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .warning-copy strong {
    color: var(--color-accent-bright);
    font-size: 12px;
  }

  .warning-copy span {
    color: var(--color-text-secondary);
  }

  @media (max-width: 720px) {
    .headers-row {
      flex-direction: column;
      align-items: stretch;
    }

    .headers-actions {
      justify-content: flex-start;
      flex-wrap: wrap;
    }
  }
</style>
