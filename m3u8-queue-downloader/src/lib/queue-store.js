import { invoke } from '@tauri-apps/api/core';
import { normalizeTaskProgress } from './progress.js';
import {
  createSessionProgressState,
  trackSessionTask as trackSessionTaskState,
} from './session-progress.js';
import { derived, writable } from 'svelte/store';

export const tasks = writable([]);
export const queueRunning = writable(false);
export const sessionProgress = writable(createSessionProgressState());
export const sessionCompletedCount = derived(
  sessionProgress,
  ($sessionProgress) => $sessionProgress.completedCount,
);

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

export function trackSessionTask(taskId) {
  sessionProgress.update((state) => trackSessionTaskState(state, taskId));
}
