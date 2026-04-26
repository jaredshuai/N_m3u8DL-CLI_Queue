import { listen } from '@tauri-apps/api/event';
import { buildProgressPatch } from './progress.js';
import { prependHistoricalTask } from './history-store.js';
import { loadQueueState, tasks } from './queue-store.js';
import {
  clearAppNotice,
  clearShutdownNotice,
  showAppErrorNotice,
  shutdownNotice,
  startShutdownCountdown,
} from './settings-store.js';
import {
  queueTerminalActiveLine,
  queueTerminalCommittedLine,
  resetTerminalLiveState,
} from './terminal-live.js';

let unlisteners = [];
let pendingProgress = {};
let progressTimer = null;

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

function flushPendingProgress() {
  if (Object.keys(pendingProgress).length === 0) return;
  if (progressTimer) {
    clearTimeout(progressTimer);
  }
  flushProgress();
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
    flushPendingProgress();
    await loadQueueState();
  });

  const u3b = await listen('task-terminal-committed-line', (event) => {
    const payload = event.payload;
    queueTerminalCommittedLine(payload.id, payload.line ?? '');
  });

  const u3c = await listen('task-terminal-active-line', (event) => {
    const payload = event.payload;
    queueTerminalActiveLine(payload.id, payload.activeLine ?? '');
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

  const u8 = await listen('task-error', (event) => {
    showAppErrorNotice(event.payload?.message ?? '任务状态保存失败');
  });

  unlisteners = [u1, u2, u3b, u3c, u4, u5, u6, u7, u8];
}

export function teardownListeners() {
  for (const unlisten of unlisteners) unlisten();
  unlisteners = [];
  if (progressTimer) {
    clearTimeout(progressTimer);
    progressTimer = null;
    pendingProgress = {};
  }
  resetTerminalLiveState();
  clearShutdownNotice();
  clearAppNotice();
}
