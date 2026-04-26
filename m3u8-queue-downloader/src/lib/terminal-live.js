import { writable } from 'svelte/store';

export const TERMINAL_ACTIVE_LINE_FLUSH_MS = 150;
export const terminalActiveLines = writable({});

export function applyTerminalActiveLineBatch(currentLines = {}, batch = {}) {
  return {
    ...(currentLines ?? {}),
    ...(batch ?? {}),
  };
}

export function createTerminalActiveLineDispatcher({
  delay = TERMINAL_ACTIVE_LINE_FLUSH_MS,
  schedule = setTimeout,
  cancel = clearTimeout,
  onFlush,
}) {
  let pending = {};
  let timer = null;

  function flush() {
    const batch = pending;
    pending = {};
    timer = null;
    if (Object.keys(batch).length > 0) {
      onFlush(batch);
    }
  }

  return {
    queue(taskId, activeLine) {
      if (!taskId) return;

      pending = {
        ...pending,
        [taskId]: activeLine ?? '',
      };

      if (timer == null) {
        timer = schedule(flush, delay);
      }
    },
    flush,
    dispose() {
      if (timer != null) {
        cancel(timer);
      }
      timer = null;
      pending = {};
    },
  };
}

const terminalActiveLineDispatcher = createTerminalActiveLineDispatcher({
  onFlush(batch) {
    terminalActiveLines.update((currentLines) =>
      applyTerminalActiveLineBatch(currentLines, batch),
    );
  },
});

export function queueTerminalActiveLine(taskId, activeLine) {
  terminalActiveLineDispatcher.queue(taskId, activeLine);
}

export function flushTerminalActiveLines() {
  terminalActiveLineDispatcher.flush();
}

export function resetTerminalActiveLines() {
  terminalActiveLineDispatcher.dispose();
  terminalActiveLines.set({});
}
