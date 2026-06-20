export function createCliConsoleState() {
  return {
    open: false,
    taskId: null,
  };
}

export const MAX_RENDERED_TERMINAL_LINES = 3000;

export function openCliConsole(currentState, taskId) {
  return {
    ...(currentState ?? createCliConsoleState()),
    open: true,
    taskId,
  };
}

export function closeCliConsole(currentState) {
  return {
    ...(currentState ?? createCliConsoleState()),
    open: false,
    taskId: null,
  };
}

export function shouldReloadTerminalState(task, loadedTaskId, loadedTaskStatus) {
  if (!task?.id) {
    return false;
  }

  if (task.id !== loadedTaskId) {
    return true;
  }

  return task.status !== loadedTaskStatus;
}

export function shouldApplyTerminalResponse(requestId, activeRequestId) {
  return requestId === activeRequestId;
}

export function shouldApplyCliOutputPageResponse({
  responseTaskId,
  currentTaskId,
  requestId,
  activeRequestId,
} = {}) {
  return (
    Boolean(responseTaskId) &&
    responseTaskId === currentTaskId &&
    shouldApplyTerminalResponse(requestId, activeRequestId)
  );
}

export function createTerminalLoadState() {
  return {
    requestId: 0,
    taskId: null,
    taskStatus: null,
  };
}

export function shouldStartTerminalStateLoad(task, loadState) {
  const current = loadState ?? createTerminalLoadState();
  return shouldReloadTerminalState(task, current.taskId, current.taskStatus);
}

export function beginTerminalStateLoad(loadState, task) {
  const current = loadState ?? createTerminalLoadState();
  return {
    requestId: current.requestId + 1,
    taskId: task?.id ?? null,
    taskStatus: task?.status ?? null,
  };
}

export function resolveTerminalActiveLine(task, persistedActiveLine = '') {
  if (task && Object.prototype.hasOwnProperty.call(task, 'terminalActiveLine')) {
    return task.terminalActiveLine || persistedActiveLine || '';
  }
  return persistedActiveLine ?? '';
}

export function findCliConsoleTask(currentState, taskGroups = {}) {
  const taskId = currentState?.taskId;
  if (!currentState?.open || !taskId) {
    return null;
  }

  const groups = [
    ...(taskGroups.tasks ?? []),
    ...(taskGroups.completedTasks ?? []),
    ...(taskGroups.failedTasks ?? []),
  ];

  return groups.find((task) => task.id === taskId) ?? null;
}

/**
 * Build the terminal view from a task's committed log lines and active line.
 * Returns { committedLines: string[], activeLine: string }.
 */
export function buildTerminalView(task) {
  const committed = Array.isArray(task?.terminalCommittedLines)
    ? [...task.terminalCommittedLines]
    : [];
  const activeLine = resolveTerminalActiveLine(task);
  return { committedLines: committed, activeLine };
}

export function capRenderedTerminalLines(
  lines = [],
  maxLines = MAX_RENDERED_TERMINAL_LINES,
) {
  if (!Array.isArray(lines)) return [];
  if (lines.length <= maxLines) return lines;
  return lines.slice(lines.length - maxLines);
}

export function findCommittedLineOverlap(persisted = [], live = []) {
  if (!Array.isArray(persisted) || !Array.isArray(live)) return 0;
  if (persisted.length === 0 || live.length === 0) return 0;

  const separator = Symbol('terminal-overlap-separator');
  const sequence = [...live, separator, ...persisted];
  const prefix = new Array(sequence.length).fill(0);

  for (let index = 1; index < sequence.length; index += 1) {
    let candidate = prefix[index - 1];
    while (candidate > 0 && sequence[index] !== sequence[candidate]) {
      candidate = prefix[candidate - 1];
    }
    if (sequence[index] === sequence[candidate]) {
      candidate += 1;
    }
    prefix[index] = candidate;
  }

  return Math.min(prefix[prefix.length - 1] ?? 0, persisted.length, live.length);
}

function mergeTerminalCommittedLinesWithOverlap(persisted = [], live = [], overlap = 0) {
  const persistedLines = Array.isArray(persisted) ? persisted : [];
  const liveLines = Array.isArray(live) ? live : [];

  if (persistedLines.length === 0) return liveLines;
  if (liveLines.length === 0) return persistedLines;

  if (overlap >= liveLines.length) {
    return persistedLines;
  }

  return [...persistedLines, ...liveLines.slice(overlap)];
}

export function mergeTerminalCommittedLines(persisted = [], live = []) {
  const persistedLines = Array.isArray(persisted) ? persisted : [];
  const liveLines = Array.isArray(live) ? live : [];
  const overlap = findCommittedLineOverlap(persistedLines, liveLines);
  return mergeTerminalCommittedLinesWithOverlap(persistedLines, liveLines, overlap);
}

function isLinePrefix(prefix = [], lines = []) {
  if (!Array.isArray(prefix) || !Array.isArray(lines)) return false;
  if (prefix.length > lines.length) return false;

  for (let index = 0; index < prefix.length; index += 1) {
    if (prefix[index] !== lines[index]) return false;
  }

  return true;
}

export function createTerminalCommittedLineMerger({
  findOverlap = findCommittedLineOverlap,
} = {}) {
  let previousPersisted = null;
  let previousLive = null;
  let previousOverlap = 0;
  let previousMerged = null;

  return function mergeCommittedLines(persisted = [], live = []) {
    const persistedLines = Array.isArray(persisted) ? persisted : [];
    const liveLines = Array.isArray(live) ? live : [];

    if (persistedLines === previousPersisted && liveLines === previousLive) {
      return previousMerged ?? mergeTerminalCommittedLines(persistedLines, liveLines);
    }

    const overlap =
      persistedLines === previousPersisted &&
      previousLive?.length > 0 &&
      isLinePrefix(previousLive, liveLines)
        ? previousOverlap
        : findOverlap(persistedLines, liveLines);
    const merged = mergeTerminalCommittedLinesWithOverlap(persistedLines, liveLines, overlap);

    previousPersisted = persistedLines;
    previousLive = liveLines;
    previousOverlap = overlap;
    previousMerged = merged;

    return merged;
  };
}
