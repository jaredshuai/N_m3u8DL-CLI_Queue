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
  let activeLoad = null;
  let reloadRequested = false;

  async function runLatestQueueStateLoad() {
    while (true) {
      reloadRequested = false;

      try {
        const state = await invokeQueueState();
        if (reloadRequested) continue;

        const normalized = (state.tasks ?? []).map(normalizeTaskProgress);
        setTasks(normalized);
        setQueueRunning(state.isRunning ?? false);
        return true;
      } catch (err) {
        if (reloadRequested) continue;

        onError('Failed to load queue state:', err);
        return false;
      }
    }
  }

  return async function loadLatestQueueState() {
    if (activeLoad) {
      reloadRequested = true;
      return activeLoad;
    }

    activeLoad = runLatestQueueStateLoad().finally(() => {
      activeLoad = null;
    });
    return activeLoad;
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
