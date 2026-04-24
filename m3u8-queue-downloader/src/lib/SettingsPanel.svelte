<script>
  import { appSettings, saveAppSettings } from './stores.js';

  let saving = $state(false);
  let error = $state('');
  let downloadDirDraft = $state('');

  async function updateSetting(patch) {
    saving = true;
    error = '';
    const nextSettings = {
      ...$appSettings,
      ...patch,
    };

    try {
      await saveAppSettings(nextSettings);
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }

  function resetDownloadDir() {
    updateSetting({ downloadDir: '' });
  }

  function commitDownloadDir() {
    if (downloadDirDraft === ($appSettings.downloadDir ?? '')) {
      return;
    }
    updateSetting({ downloadDir: downloadDirDraft });
  }

  function handleDownloadDirKeydown(event) {
    if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      event.currentTarget.blur();
    }
  }

  $effect(() => {
    downloadDirDraft = $appSettings.downloadDir ?? '';
  });
</script>

<section class="settings-panel fade-in" aria-label="设置">
  <div class="settings-header">
    <div>
      <h2>设置</h2>
      <p>窗口、托盘与完成后动作</p>
    </div>
    {#if saving}
      <span class="saving">保存中...</span>
    {/if}
  </div>

  <label class="setting-row" for="close-behavior">
    <div class="setting-copy">
      <strong>关闭按钮默认行为</strong>
      <span>关闭到托盘会保留队列与托盘菜单；直接退出会结束程序。</span>
    </div>
    <select
      id="close-behavior"
      value={$appSettings.closeButtonBehavior}
      onchange={(event) => updateSetting({ closeButtonBehavior: event.currentTarget.value })}
      disabled={saving}
    >
      <option value="closeToTray">关闭到托盘</option>
      <option value="exit">直接退出程序</option>
    </select>
  </label>

  <label class="setting-row checkbox-row">
    <div class="setting-copy">
      <strong>所有任务完成后自动关闭电脑</strong>
      <span>仅在没有等待/下载中的任务且本轮没有失败任务时触发。</span>
    </div>
    <input
      type="checkbox"
      checked={$appSettings.autoShutdownOnComplete}
      onchange={(event) => updateSetting({ autoShutdownOnComplete: event.currentTarget.checked })}
      disabled={saving}
    />
  </label>

  <div class="setting-row download-dir-row">
    <div class="setting-copy">
      <strong>下载目录</strong>
      <span>留空时自动使用 Windows 视频文件夹；也可以填入自定义目录。</span>
    </div>
    <div class="download-dir-controls">
      <input
        type="text"
        bind:value={downloadDirDraft}
        onkeydown={handleDownloadDirKeydown}
        onblur={commitDownloadDir}
        placeholder="留空使用系统视频文件夹"
        disabled={saving}
      />
      <button
        type="button"
        class="secondary-btn"
        onclick={resetDownloadDir}
        disabled={saving || !($appSettings.downloadDir ?? '').trim()}
      >
        恢复自动
      </button>
    </div>
  </div>

  {#if error}
    <div class="settings-error">{error}</div>
  {/if}
</section>

<style>
  .settings-panel {
    border-bottom: 1px solid var(--color-border);
    background: rgba(13, 16, 22, 0.96);
    padding: 14px 16px 12px;
    box-shadow: 0 12px 28px rgba(0,0,0,0.25);
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: flex-start;
    margin-bottom: 12px;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 14px;
    color: var(--color-text-main);
  }

  .settings-header p {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--color-text-secondary);
  }

  .saving {
    font-size: 12px;
    color: var(--color-accent);
  }

  .setting-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 190px;
    gap: 16px;
    align-items: center;
    padding: 12px 0;
    border-top: 1px solid rgba(255,255,255,0.05);
  }

  .checkbox-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .download-dir-row {
    grid-template-columns: minmax(0, 1fr) minmax(260px, 420px);
    align-items: start;
  }

  .setting-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .setting-copy strong {
    color: var(--color-text-main);
    font-size: 13px;
  }

  .setting-copy span {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  select {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-input);
    color: var(--color-text-main);
    font-family: var(--font-stack);
    outline: none;
  }

  select:focus {
    border-color: var(--color-accent);
  }

  .download-dir-controls {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .download-dir-controls input {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-input);
    color: var(--color-text-main);
    font-family: var(--font-stack);
    outline: none;
  }

  .download-dir-controls input:focus {
    border-color: var(--color-accent);
  }

  .secondary-btn {
    flex-shrink: 0;
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: rgba(255,255,255,0.03);
    color: var(--color-text-secondary);
    font-family: var(--font-stack);
    cursor: pointer;
  }

  .secondary-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  input[type='checkbox'] {
    width: 38px;
    height: 20px;
    accent-color: var(--color-accent);
  }

  .settings-error {
    margin-top: 8px;
    padding: 8px 10px;
    border: 1px solid rgba(248, 113, 113, 0.35);
    border-radius: var(--radius-sm);
    background: rgba(248, 113, 113, 0.08);
    color: var(--color-status-fail);
    font-size: 12px;
  }
</style>
