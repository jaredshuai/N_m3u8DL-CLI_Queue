import { invoke } from '@tauri-apps/api/core';
import { normalizeTaskProgress } from './progress.js';
import {
  createSessionProgressState,
  trackSessionTask as trackSessionTaskState,
} from './session-progress.js';
import { showAppErrorNotice } from './settings-store.js';
import { derived, writable } from 'svelte/store';

export const tasks = writable([]);
export const queueRunning = writable(false);
export const currentTaskId = writable(null);
export const sessionProgress = writable(createSessionProgressState());
export const sessionCompletedCount = derived(
  sessionProgress,
  ($sessionProgress) => $sessionProgress.completedCount,
);

export function createQueueStateLoader({
  invokeQueueState,
  setTasks,
  setQueueRunning,
  setCurrentTaskId = () => {},
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
        setCurrentTaskId(state.currentTaskId ?? null);
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
  setCurrentTaskId: (taskId) => currentTaskId.set(taskId),
});

export async function loadQueueState() {
  return loadLatestQueueState();
}

export async function runStopTask(
  taskId,
  { invokeCommand, reloadQueueState, onError = () => {} },
) {
  try {
    await invokeCommand('stop_task', { taskId });
  } catch (error) {
    onError(error);
    throw error;
  } finally {
    // Backend emits a queue-state-changed event via the lifecycle handler, but
    // proactively re-read state so the UI reflects Cancelled immediately even
    // if the lifecycle event races or is delayed.
    await reloadQueueState();
  }
}

export async function stopTask(taskId) {
  return runStopTask(taskId, {
    invokeCommand: invoke,
    reloadQueueState: loadQueueState,
    onError: (error) => showAppErrorNotice(error, '停止任务失败'),
  });
}

export function isTaskTerminationPending(task, activeTaskId) {
  return task?.status === 'cancelled' && task.id === activeTaskId;
}

export function trackSessionTask(taskId) {
  sessionProgress.update((state) => trackSessionTaskState(state, taskId));
}
