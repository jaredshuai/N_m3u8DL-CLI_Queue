import { writable } from 'svelte/store';

export const TERMINAL_ACTIVE_LINE_FLUSH_MS = 150;
export const TERMINAL_COMMITTED_LINES_FLUSH_MS = 150;
export const MAX_LIVE_TERMINAL_COMMITTED_LINES = 2000;
export const terminalActiveLines = writable({});
export const terminalCommittedLines = writable({});

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

export function appendTerminalCommittedLineBatch(
  currentLines = {},
  batch = {},
  maxLines = MAX_LIVE_TERMINAL_COMMITTED_LINES,
) {
  const next = { ...(currentLines ?? {}) };

  for (const [taskId, lines] of Object.entries(batch ?? {})) {
    if (!taskId || !Array.isArray(lines) || lines.length === 0) continue;

    const existing = Array.isArray(next[taskId]) ? next[taskId] : [];
    const merged = [...existing, ...lines];
    next[taskId] =
      merged.length > maxLines ? merged.slice(merged.length - maxLines) : merged;
  }

  return next;
}

export function createTerminalCommittedLineDispatcher({
  delay = TERMINAL_COMMITTED_LINES_FLUSH_MS,
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
    queue(taskId, line) {
      if (!taskId || !line) return;

      pending = {
        ...pending,
        [taskId]: [...(pending[taskId] ?? []), line],
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

const terminalCommittedLineDispatcher = createTerminalCommittedLineDispatcher({
  onFlush(batch) {
    terminalCommittedLines.update((currentLines) =>
      appendTerminalCommittedLineBatch(currentLines, batch),
    );
  },
});

export function queueTerminalActiveLine(taskId, activeLine) {
  terminalActiveLineDispatcher.queue(taskId, activeLine);
}

export function queueTerminalCommittedLine(taskId, line) {
  terminalCommittedLineDispatcher.queue(taskId, line);
}

export function flushTerminalActiveLines() {
  terminalActiveLineDispatcher.flush();
}

export function flushTerminalCommittedLines() {
  terminalCommittedLineDispatcher.flush();
}

export function resetTerminalActiveLines() {
  terminalActiveLineDispatcher.dispose();
  terminalActiveLines.set({});
}

export function resetTerminalCommittedLines() {
  terminalCommittedLineDispatcher.dispose();
  terminalCommittedLines.set({});
}

export function resetTerminalLiveState() {
  resetTerminalActiveLines();
  resetTerminalCommittedLines();
}
