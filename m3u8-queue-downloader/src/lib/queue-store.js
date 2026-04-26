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

export function createQueueStateLoader({
  invokeQueueState,
  setTasks,
  setQueueRunning,
  onError = console.error,
}) {
  let requestGeneration = 0;

  return async function loadLatestQueueState() {
    const requestId = requestGeneration + 1;
    requestGeneration = requestId;

    try {
      const state = await invokeQueueState();
      if (requestId !== requestGeneration) return false;

      const normalized = (state.tasks ?? []).map(normalizeTaskProgress);
      setTasks(normalized);
      setQueueRunning(state.isRunning ?? false);
      return true;
    } catch (err) {
      if (requestId === requestGeneration) {
        onError('Failed to load queue state:', err);
      }
      return false;
    }
  };
}

const loadLatestQueueState = createQueueStateLoader({
  invokeQueueState: () => invoke('get_queue_state'),
  setTasks: (nextTasks) => tasks.set(nextTasks),
  setQueueRunning: (running) => queueRunning.set(running),
});

export async function loadQueueState() {
  return loadLatestQueueState();
}

export function trackSessionTask(taskId) {
  sessionProgress.update((state) => trackSessionTaskState(state, taskId));
}
