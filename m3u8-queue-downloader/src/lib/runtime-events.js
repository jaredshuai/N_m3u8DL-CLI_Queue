import { listen } from '@tauri-apps/api/event';
import { buildProgressPatch, normalizeTaskProgress } from './progress.js';
import { prependHistoricalTask, completedHistory, failedHistory } from './history-store.js';
import { loadQueueState, sessionProgress, tasks } from './queue-store.js';
import { clearShutdownNotice, shutdownNotice, startShutdownCountdown } from './settings-store.js';

let unlisteners = [];
let pendingProgress = {};
let progressTimer = null;

function updateTaskList(list, taskId, updater) {
  return list.map((task) => (task.id === taskId ? updater(task) : task));
}

function updateHistoryStore(store, taskId, updater) {
  store.update((state) => ({
    ...state,
    tasks: updateTaskList(state.tasks, taskId, updater),
  }));
}

function updateTaskEverywhere(taskId, updater) {
  tasks.update((currentTasks) => updateTaskList(currentTasks, taskId, updater));
  updateHistoryStore(completedHistory, taskId, updater);
  updateHistoryStore(failedHistory, taskId, updater);
}

function flushProgress() {
  const batch = pendingProgress;
  pendingProgress = {};
  progressTimer = null;
  tasks.update((currentTasks) =>
    currentTasks.map((task) => {
      const update = batch[task.id];
      return update ? { ...task, ...update } : task;
    }),
  );
}

export async function setupListeners() {
  const u1 = await listen('task-progress', (event) => {
    const payload = event.payload;
    const patch = buildProgressPatch(payload);
    if (Object.keys(patch).length === 0) return;

    const previous = pendingProgress[payload.id] || {};
    pendingProgress[payload.id] = {
      ...previous,
      ...patch,
    };
    if (!progressTimer) {
      progressTimer = setTimeout(flushProgress, 200);
    }
  });

  const u2 = await listen('queue-state-changed', async () => {
    await loadQueueState();
  });

  const u3b = await listen('task-terminal-committed-line', (event) => {
    const payload = event.payload;
    updateTaskEverywhere(payload.id, (task) => {
      const lines = [...(task.terminalCommittedLines ?? []), payload.line];
      const MAX_TERMINAL_LINES = 2000;
      return {
        ...task,
        terminalCommittedLines:
          lines.length > MAX_TERMINAL_LINES
            ? lines.slice(lines.length - MAX_TERMINAL_LINES)
            : lines,
        terminalActiveLine: task.terminalActiveLine ?? '',
      };
    });
  });

  const u3c = await listen('task-terminal-active-line', (event) => {
    const payload = event.payload;
    updateTaskEverywhere(payload.id, (task) => ({
      ...task,
      terminalActiveLine: payload.activeLine ?? '',
    }));
  });

  const u4 = await listen('history-task-added', (event) => {
    const payload = event.payload;
    prependHistoricalTask(payload.status, payload.task);
  });

  const u5 = await listen('shutdown-countdown-started', (event) => {
    const seconds = Number(event.payload?.seconds ?? 60);
    startShutdownCountdown(seconds);
  });

  const u6 = await listen('shutdown-countdown-cancelled', () => {
    clearShutdownNotice();
  });

  const u7 = await listen('shutdown-error', (event) => {
    shutdownNotice.set({
      active: false,
      secondsRemaining: 0,
      error: event.payload?.message ?? '自动关机启动失败',
    });
  });

  unlisteners = [u1, u2, u3b, u3c, u4, u5, u6, u7];
}

export function teardownListeners() {
  for (const unlisten of unlisteners) unlisten();
  unlisteners = [];
  if (progressTimer) {
    clearTimeout(progressTimer);
    progressTimer = null;
    pendingProgress = {};
  }
  clearShutdownNotice();
}
