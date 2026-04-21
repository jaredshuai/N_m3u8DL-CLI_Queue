export function createSessionProgressState() {
  return {
    pendingTaskIds: [],
    completedCount: 0,
  };
}

export function trackSessionTask(state, taskId) {
  if (!taskId || state.pendingTaskIds.includes(taskId)) {
    return state;
  }

  return {
    ...state,
    pendingTaskIds: [...state.pendingTaskIds, taskId],
  };
}

export function recordHistoricalSessionTask(state, task) {
  if (!task?.id || !state.pendingTaskIds.includes(task.id)) {
    return state;
  }

  const pendingTaskIds = state.pendingTaskIds.filter((id) => id !== task.id);
  if (task.status === 'completed') {
    return {
      pendingTaskIds,
      completedCount: state.completedCount + 1,
    };
  }

  return {
    ...state,
    pendingTaskIds,
  };
}
