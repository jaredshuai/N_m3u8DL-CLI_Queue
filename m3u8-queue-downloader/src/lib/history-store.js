import { invoke } from '@tauri-apps/api/core';
import {
  createHistoryState,
  DEFAULT_HISTORY_PAGE_SIZE,
  mergeHistoryPage,
  prependHistoryTask,
  removeHistoryTask,
} from './history.js';
import { normalizeTaskProgress } from './progress.js';
import { sessionProgress } from './queue-store.js';
import { recordHistoricalSessionTask } from './session-progress.js';
import { get, writable } from 'svelte/store';

export const completedHistory = writable(createHistoryState());
export const failedHistory = writable(createHistoryState());

function historyStore(status) {
  return status === 'completed' ? completedHistory : failedHistory;
}

function normalizeTask(task) {
  return normalizeTaskProgress(task);
}

export async function loadHistoryPage(
  status,
  { reset = false, limit = DEFAULT_HISTORY_PAGE_SIZE } = {},
) {
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

export async function clearHistoryTask(status, taskId) {
  try {
    await invoke('remove_history_task', { status, taskId });
    const store = historyStore(status);
    store.update((state) => removeHistoryTask(state, taskId));
  } catch (err) {
    console.error(`Failed to remove ${status} history task:`, err);
    throw err;
  }
}

export function prependHistoricalTask(status, task) {
  const store = historyStore(status);
  store.update((state) =>
    prependHistoryTask(state, {
      ...normalizeTask(task),
      terminalActiveLine: '',
      terminalCommittedLines: [],
    }),
  );
  sessionProgress.update((state) => recordHistoricalSessionTask(state, task));
}
