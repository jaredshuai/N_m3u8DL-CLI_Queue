export function toDndItems(waitingTasks = []) {
  return waitingTasks.map((task) => ({ ...task, id: task.id }));
}

export function getTaskIdSignature(items = []) {
  return items.map((item) => item.id).join('|');
}

export function shouldSyncDndItems({
  waitingTasks = [],
  dndItems = [],
  syncLocked = false,
} = {}) {
  if (syncLocked) {
    return false;
  }

  return getTaskIdSignature(waitingTasks) !== getTaskIdSignature(dndItems);
}
