<script>
  import { appSettings, saveAppSettings } from './stores.js';

  let saving = $state(false);
  let error = $state('');

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
</script>

<section class="settings-panel fade-in" aria-label="Settings">
  <div class="settings-header">
    <div>
      <h2>Settings</h2>
      <p>Window, tray, and completion behavior.</p>
    </div>
    {#if saving}
      <span class="saving">Saving...</span>
    {/if}
  </div>

  <label class="setting-row" for="close-behavior">
    <div class="setting-copy">
      <strong>Default close button behavior</strong>
      <span>Hide to tray keeps the queue alive; exit closes the app process.</span>
    </div>
    <select
      id="close-behavior"
      value={$appSettings.closeButtonBehavior}
      onchange={(event) => updateSetting({ closeButtonBehavior: event.currentTarget.value })}
      disabled={saving}
    >
      <option value="closeToTray">Close to tray</option>
      <option value="exit">Exit app</option>
    </select>
  </label>

  <label class="setting-row checkbox-row">
    <div class="setting-copy">
      <strong>Auto action after all tasks complete</strong>
      <span>Only triggers when there are no waiting/downloading tasks and the run has no failures.</span>
    </div>
    <input
      type="checkbox"
      checked={$appSettings.autoShutdownOnComplete}
      onchange={(event) => updateSetting({ autoShutdownOnComplete: event.currentTarget.checked })}
      disabled={saving}
    />
  </label>

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
