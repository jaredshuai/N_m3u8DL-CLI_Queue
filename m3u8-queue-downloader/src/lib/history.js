export const DEFAULT_HISTORY_PAGE_SIZE = 20;

export function createHistoryState() {
  return {
    tasks: [],
    hasMore: false,
    nextOffset: 0,
  };
}

export function mergeHistoryPage(currentState, page, reset = false) {
  const current = currentState ?? createHistoryState();
  return {
    tasks: reset ? [...page.tasks] : [...current.tasks, ...page.tasks],
    hasMore: page.hasMore,
    nextOffset: page.nextOffset,
  };
}

export function prependHistoryTask(currentState, task) {
  const current = currentState ?? createHistoryState();
  const visibleCount = Math.max(current.tasks.length, DEFAULT_HISTORY_PAGE_SIZE);
  const alreadyVisible = current.tasks.some((item) => item.id === task.id);
  const nextTasks = [task, ...current.tasks.filter((item) => item.id !== task.id)];
  const visibleWindowGrew = !alreadyVisible && current.tasks.length < visibleCount;

  return {
    ...current,
    nextOffset: visibleWindowGrew ? current.nextOffset + 1 : current.nextOffset,
    tasks: nextTasks.slice(0, visibleCount),
    hasMore: current.hasMore || (!alreadyVisible && current.tasks.length >= visibleCount),
  };
}

export function removeHistoryTask(currentState, taskId) {
  const current = currentState ?? createHistoryState();
  const existed = current.tasks.some((task) => task.id === taskId);

  if (!existed) {
    return current;
  }

  return {
    ...current,
    tasks: current.tasks.filter((task) => task.id !== taskId),
    nextOffset: Math.max(0, current.nextOffset - 1),
  };
}
