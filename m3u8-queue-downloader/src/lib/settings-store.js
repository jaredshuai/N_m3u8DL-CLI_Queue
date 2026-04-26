import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export const appSettings = writable({
  closeButtonBehavior: 'closeToTray',
  autoShutdownOnComplete: false,
  downloadDir: '',
});

export const shutdownNotice = writable({
  active: false,
  secondsRemaining: 0,
  error: null,
});

export const appNotice = writable({
  title: '',
  message: '',
});

let shutdownTimer = null;

function clearShutdownTimer() {
  if (shutdownTimer) {
    clearInterval(shutdownTimer);
    shutdownTimer = null;
  }
}

export function startShutdownCountdown(seconds) {
  clearShutdownTimer();
  shutdownNotice.set({
    active: true,
    secondsRemaining: seconds,
    error: null,
  });

  shutdownTimer = setInterval(() => {
    shutdownNotice.update((notice) => {
      const nextSeconds = Math.max(0, notice.secondsRemaining - 1);
      if (nextSeconds === 0) {
        clearShutdownTimer();
      }
      return {
        ...notice,
        secondsRemaining: nextSeconds,
      };
    });
  }, 1000);
}

export async function loadAppSettings() {
  try {
    const settings = await invoke('get_app_settings');
    appSettings.set(settings);
  } catch (err) {
    console.error('Failed to load app settings:', err);
  }
}

export async function saveAppSettings(settings) {
  try {
    const updated = await invoke('update_app_settings', { settings });
    appSettings.set(updated);
    return updated;
  } catch (err) {
    console.error('Failed to save app settings:', err);
    throw err;
  }
}

export async function cancelAutoShutdown() {
  try {
    await invoke('cancel_auto_shutdown');
    clearShutdownTimer();
    shutdownNotice.set({
      active: false,
      secondsRemaining: 0,
      error: null,
    });
  } catch (err) {
    shutdownNotice.set({
      active: false,
      secondsRemaining: 0,
      error: String(err),
    });
  }
}

export function clearShutdownNotice() {
  clearShutdownTimer();
  shutdownNotice.set({
    active: false,
    secondsRemaining: 0,
    error: null,
  });
}

export function showAppErrorNotice(message, title = '任务状态保存失败') {
  appNotice.set({
    title,
    message: String(message || '任务状态保存失败'),
  });
}

export function clearAppNotice() {
  appNotice.set({
    title: '',
    message: '',
  });
}
