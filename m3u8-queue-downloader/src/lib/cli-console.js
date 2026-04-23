export function createCliConsoleState() {
  return {
    open: false,
    taskId: null,
  };
}

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

export function resolveTerminalActiveLine(task, persistedActiveLine = '') {
  if (task && Object.prototype.hasOwnProperty.call(task, 'terminalActiveLine')) {
    return task.terminalActiveLine ?? '';
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
