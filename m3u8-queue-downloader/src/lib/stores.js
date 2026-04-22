import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  createHistoryState,
  DEFAULT_HISTORY_PAGE_SIZE,
  mergeHistoryPage,
  prependHistoryTask,
} from './history.js';
import { appendLogLine } from './logs.js';
import { buildProgressPatch, normalizeTaskProgress } from './progress.js';
import {
  createSessionProgressState,
  recordHistoricalSessionTask,
  trackSessionTask as trackSessionTaskState,
} from './session-progress.js';
import { derived, get, writable } from 'svelte/store';

export const tasks = writable([]);
export const queueRunning = writable(false);
export const completedHistory = writable(createHistoryState());
export const failedHistory = writable(createHistoryState());
export const sessionProgress = writable(createSessionProgressState());
export const appSettings = writable({
  closeButtonBehavior: 'closeToTray',
  autoShutdownOnComplete: false,
});
export const shutdownNotice = writable({
  active: false,
  secondsRemaining: 0,
  error: null,
});
export const sessionCompletedCount = derived(
  sessionProgress,
  ($sessionProgress) => $sessionProgress.completedCount,
);

let unlisteners = [];
let pendingProgress = {};
let progressTimer = null;

function flushProgress() {
  const batch = pendingProgress;
  pendingProgress = {};
  progressTimer = null;
  tasks.update((currentTasks) => currentTasks.map(t => {
    const update = batch[t.id];
    return update ? { ...t, ...update } : t;
  }));
}

export async function loadQueueState() {
  try {
    const state = await invoke('get_queue_state');
    const normalized = (state.tasks ?? []).map(normalizeTaskProgress);
    tasks.set(normalized);
    queueRunning.set(state.isRunning ?? false);
  } catch (err) {
    console.error('Failed to load queue state:', err);
  }
}

function historyStore(status) {
  return status === 'completed' ? completedHistory : failedHistory;
}

function normalizeTask(task) {
  return normalizeTaskProgress(task);
}

export async function loadHistoryPage(status, { reset = false, limit = DEFAULT_HISTORY_PAGE_SIZE } = {}) {
  try {
    const store = historyStore(status);
    const currentState = get(store);
    const offset = reset ? 0 : currentState.nextOffset;
    const page = await invoke('get_history_page', { status, offset, limit });
    const normalizedPage = {
      ...page,
      tasks: (page.tasks ?? []).map(normalizeTask),
    };
    store.update((state) => mergeHistoryPage(state, normalizedPage, reset));
  } catch (err) {
    console.error(`Failed to load ${status} history:`, err);
  }
}

export async function loadInitialHistory() {
  await Promise.all([
    loadHistoryPage('completed', { reset: true }),
    loadHistoryPage('failed', { reset: true }),
  ]);
}

export function trackSessionTask(taskId) {
  sessionProgress.update((state) => trackSessionTaskState(state, taskId));
}

let shutdownTimer = null;

function clearShutdownTimer() {
  if (shutdownTimer) {
    clearInterval(shutdownTimer);
    shutdownTimer = null;
  }
}

function startShutdownCountdown(seconds) {
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

export async function setupListeners() {
  const u1 = await listen('task-progress', (event) => {
    const d = event.payload;
    const patch = buildProgressPatch(d);
    if (Object.keys(patch).length === 0) return;

    const prev = pendingProgress[d.id] || {};
    pendingProgress[d.id] = {
      ...prev,
      ...patch,
    };
    if (!progressTimer) {
      progressTimer = setTimeout(flushProgress, 200);
    }
  });

  const u2 = await listen('queue-state-changed', async () => {
    await loadQueueState();
  });

  const u3 = await listen('task-log', (event) => {
    const d = event.payload;
    tasks.update((currentTasks) => currentTasks.map(t =>
      t.id === d.id
        ? { ...t, logLines: appendLogLine(t.logLines, d.line) }
        : t
    ));
  });

  const u4 = await listen('history-task-added', (event) => {
    const d = event.payload;
    const store = historyStore(d.status);
    store.update((state) => prependHistoryTask(state, normalizeTask(d.task)));
    sessionProgress.update((state) => recordHistoricalSessionTask(state, d.task));
  });

  const u5 = await listen('shutdown-countdown-started', (event) => {
    const seconds = Number(event.payload?.seconds ?? 60);
    startShutdownCountdown(seconds);
  });

  const u6 = await listen('shutdown-countdown-cancelled', () => {
    clearShutdownTimer();
    shutdownNotice.set({
      active: false,
      secondsRemaining: 0,
      error: null,
    });
  });

  const u7 = await listen('shutdown-error', (event) => {
    clearShutdownTimer();
    shutdownNotice.set({
      active: false,
      secondsRemaining: 0,
      error: event.payload?.message ?? '自动关机启动失败',
    });
  });

  unlisteners = [u1, u2, u3, u4, u5, u6, u7];
}

export function teardownListeners() {
  for (const u of unlisteners) u();
  unlisteners = [];
  if (progressTimer) {
    clearTimeout(progressTimer);
    progressTimer = null;
    pendingProgress = {};
  }
  clearShutdownTimer();
}
