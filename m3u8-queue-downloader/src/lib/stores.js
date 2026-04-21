import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  createHistoryState,
  DEFAULT_HISTORY_PAGE_SIZE,
  mergeHistoryPage,
  prependHistoryTask,
} from './history.js';
import { appendLogLine } from './logs.js';
import {
  createSessionProgressState,
  recordHistoricalSessionTask,
  trackSessionTask as trackSessionTaskState,
} from './session-progress.js';
import { derived, get, writable } from 'svelte/store';

/**
 * Task object shape:
 * {
 *   id: string,
 *   url: string,
 *   save_name: string | null,
 *   headers: string | null,
 *   status: "waiting" | "downloading" | "completed" | "failed",
 *   progress: number,       // 0-100
 *   speed: string | null,   // e.g. "2.5 MB/s"
 *   threads: string | null, // e.g. "8/16"
 *   log: string[],          // CLI output lines
 *   output_path: string | null,
 *   error_message: string | null,
 * }
 */

export const tasks = writable([]);
export const queueRunning = writable(false);
export const completedHistory = writable(createHistoryState());
export const failedHistory = writable(createHistoryState());
export const sessionProgress = writable(createSessionProgressState());
export const sessionCompletedCount = derived(
  sessionProgress,
  ($sessionProgress) => $sessionProgress.completedCount,
);

/** Unsubscriber functions for event listeners */
let unlisteners = [];

/** Pending progress updates batched by task ID */
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

/** Load full queue state from backend */
export async function loadQueueState() {
  try {
    const state = await invoke('get_queue_state');
    // Backend progress is 0.0~1.0, frontend expects 0~100
    const normalized = (state.tasks ?? []).map(t => ({
      ...t,
      progress: (t.progress != null && t.progress >= 0) ? t.progress * 100 : 0,
    }));
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
  return {
    ...task,
    progress: (task.progress != null && task.progress >= 0) ? task.progress * 100 : 0,
  };
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

/** Set up all Tauri event listeners */
export async function setupListeners() {
  const u1 = await listen('task-progress', (event) => {
    const d = event.payload;
    const progress = (d.progress != null && d.progress >= 0) ? d.progress * 100 : null;
    const prev = pendingProgress[d.id] || {};
    pendingProgress[d.id] = {
      ...prev,
      ...(progress != null ? { progress } : {}),
      ...(d.speed ? { speed: d.speed } : {}),
      ...(d.threads ? { threads: d.threads } : {}),
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

  unlisteners = [u1, u2, u3, u4];
}

/** Clean up listeners (for teardown) */
export function teardownListeners() {
  for (const u of unlisteners) u();
  unlisteners = [];
  if (progressTimer) {
    clearTimeout(progressTimer);
    progressTimer = null;
    pendingProgress = {};
  }
}
