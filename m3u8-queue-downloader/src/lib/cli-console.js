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
  const activeLine = task?.terminalActiveLine ?? '';
  return { committedLines: committed, activeLine };
}
