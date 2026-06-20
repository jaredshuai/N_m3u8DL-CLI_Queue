import { writable } from 'svelte/store';

export const TERMINAL_ACTIVE_LINE_FLUSH_MS = 150;
export const TERMINAL_COMMITTED_LINES_FLUSH_MS = 150;
export const MAX_LIVE_TERMINAL_COMMITTED_LINES = 2000;

const terminalActiveLineValues = new Map();
const terminalCommittedLineValues = new Map();
const terminalActiveLineStores = new Map();
const terminalCommittedLineStores = new Map();
const terminalActiveLinesAggregate = writable({});
const terminalCommittedLinesAggregate = writable({});
let terminalActiveLinesAggregateSubscribers = 0;
let terminalCommittedLinesAggregateSubscribers = 0;

function objectFromMap(map) {
  return Object.fromEntries(map.entries());
}

function getTerminalActiveLineState(taskId) {
  const hasValue = terminalActiveLineValues.has(taskId);
  return {
    hasValue,
    line: hasValue ? (terminalActiveLineValues.get(taskId) ?? '') : '',
  };
}

function getOrCreateTerminalActiveLineStore(taskId) {
  let store = terminalActiveLineStores.get(taskId);
  if (!store) {
    store = writable(getTerminalActiveLineState(taskId));
    terminalActiveLineStores.set(taskId, store);
  }
  return store;
}

function getOrCreateTerminalCommittedLineStore(taskId) {
  let store = terminalCommittedLineStores.get(taskId);
  if (!store) {
    store = writable(terminalCommittedLineValues.get(taskId) ?? []);
    terminalCommittedLineStores.set(taskId, store);
  }
  return store;
}

function publishTerminalActiveLinesAggregate() {
  if (terminalActiveLinesAggregateSubscribers > 0) {
    terminalActiveLinesAggregate.set(objectFromMap(terminalActiveLineValues));
  }
}

function publishTerminalCommittedLinesAggregate() {
  if (terminalCommittedLinesAggregateSubscribers > 0) {
    terminalCommittedLinesAggregate.set(objectFromMap(terminalCommittedLineValues));
  }
}

function notifyTerminalActiveLineStore(taskId) {
  terminalActiveLineStores.get(taskId)?.set(getTerminalActiveLineState(taskId));
}

function notifyTerminalCommittedLineStore(taskId) {
  terminalCommittedLineStores
    .get(taskId)
    ?.set(terminalCommittedLineValues.get(taskId) ?? []);
}

function replaceTerminalActiveLineValues(nextLines = {}) {
  const changedTaskIds = new Set([
    ...terminalActiveLineValues.keys(),
    ...terminalActiveLineStores.keys(),
    ...Object.keys(nextLines ?? {}),
  ]);

  terminalActiveLineValues.clear();
  for (const [taskId, activeLine] of Object.entries(nextLines ?? {})) {
    if (!taskId) continue;
    terminalActiveLineValues.set(taskId, activeLine ?? '');
  }

  for (const taskId of changedTaskIds) {
    notifyTerminalActiveLineStore(taskId);
  }
  publishTerminalActiveLinesAggregate();
}

function replaceTerminalCommittedLineValues(nextLines = {}) {
  const changedTaskIds = new Set([
    ...terminalCommittedLineValues.keys(),
    ...terminalCommittedLineStores.keys(),
    ...Object.keys(nextLines ?? {}),
  ]);

  terminalCommittedLineValues.clear();
  for (const [taskId, lines] of Object.entries(nextLines ?? {})) {
    if (!taskId || !Array.isArray(lines)) continue;
    terminalCommittedLineValues.set(taskId, lines);
  }

  for (const taskId of changedTaskIds) {
    notifyTerminalCommittedLineStore(taskId);
  }
  publishTerminalCommittedLinesAggregate();
}

function applyTerminalActiveLineValueBatch(batch = {}) {
  for (const [taskId, activeLine] of Object.entries(batch ?? {})) {
    if (!taskId) continue;
    terminalActiveLineValues.set(taskId, activeLine ?? '');
    notifyTerminalActiveLineStore(taskId);
  }
  publishTerminalActiveLinesAggregate();
}

function appendTerminalCommittedLineValueBatch(
  batch = {},
  maxLines = MAX_LIVE_TERMINAL_COMMITTED_LINES,
) {
  for (const [taskId, lines] of Object.entries(batch ?? {})) {
    if (!taskId || !Array.isArray(lines) || lines.length === 0) continue;

    const existing = terminalCommittedLineValues.get(taskId) ?? [];
    const merged = [...existing, ...lines];
    terminalCommittedLineValues.set(
      taskId,
      merged.length > maxLines ? merged.slice(merged.length - maxLines) : merged,
    );
    notifyTerminalCommittedLineStore(taskId);
  }
  publishTerminalCommittedLinesAggregate();
}

export const terminalActiveLines = {
  subscribe(run, invalidate) {
    terminalActiveLinesAggregateSubscribers += 1;
    terminalActiveLinesAggregate.set(objectFromMap(terminalActiveLineValues));
    const unsubscribe = terminalActiveLinesAggregate.subscribe(run, invalidate);
    return () => {
      terminalActiveLinesAggregateSubscribers = Math.max(
        0,
        terminalActiveLinesAggregateSubscribers - 1,
      );
      unsubscribe();
    };
  },
  set: replaceTerminalActiveLineValues,
  update(updater) {
    replaceTerminalActiveLineValues(updater(objectFromMap(terminalActiveLineValues)));
  },
};

export const terminalCommittedLines = {
  subscribe(run, invalidate) {
    terminalCommittedLinesAggregateSubscribers += 1;
    terminalCommittedLinesAggregate.set(objectFromMap(terminalCommittedLineValues));
    const unsubscribe = terminalCommittedLinesAggregate.subscribe(run, invalidate);
    return () => {
      terminalCommittedLinesAggregateSubscribers = Math.max(
        0,
        terminalCommittedLinesAggregateSubscribers - 1,
      );
      unsubscribe();
    };
  },
  set: replaceTerminalCommittedLineValues,
  update(updater) {
    replaceTerminalCommittedLineValues(updater(objectFromMap(terminalCommittedLineValues)));
  },
};

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

      pending[taskId] = activeLine ?? '';

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

      if (!Array.isArray(pending[taskId])) {
        pending[taskId] = [];
      }
      pending[taskId].push(line);

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
    applyTerminalActiveLineValueBatch(batch);
  },
});

const terminalCommittedLineDispatcher = createTerminalCommittedLineDispatcher({
  onFlush(batch) {
    appendTerminalCommittedLineValueBatch(batch);
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

export function subscribeTerminalActiveLine(taskId, run, invalidate) {
  if (!taskId) {
    return writable({ hasValue: false, line: '' }).subscribe(run, invalidate);
  }
  return getOrCreateTerminalActiveLineStore(taskId).subscribe(run, invalidate);
}

export function subscribeTerminalCommittedLines(taskId, run, invalidate) {
  if (!taskId) {
    return writable([]).subscribe(run, invalidate);
  }
  return getOrCreateTerminalCommittedLineStore(taskId).subscribe(run, invalidate);
}

export function resetTerminalActiveLines() {
  terminalActiveLineDispatcher.dispose();
  replaceTerminalActiveLineValues({});
}

export function resetTerminalCommittedLines() {
  terminalCommittedLineDispatcher.dispose();
  replaceTerminalCommittedLineValues({});
}

export function resetTerminalLiveState() {
  resetTerminalActiveLines();
  resetTerminalCommittedLines();
}
