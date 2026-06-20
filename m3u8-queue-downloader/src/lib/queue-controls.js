async function defaultInvoke(command, args) {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke(command, args);
}

export function getQueueControlState({ tasks, queueRunning, busy }) {
  const hasTasks = (tasks ?? []).length > 0;
  const downloadingCount = (tasks ?? []).filter((task) => task.status === 'downloading').length;
  const waitingCount = (tasks ?? []).filter((task) => task.status === 'waiting').length;
  const isDraining = !queueRunning && downloadingCount > 0;

  let action = 'start';
  let label = '开始队列';
  let disabled = !hasTasks || busy;

  if (queueRunning) {
    action = 'pause';
    label = downloadingCount > 0 ? '暂停后续任务' : '暂停队列';
  } else if (isDraining && waitingCount > 0) {
    action = 'resume';
    label = '恢复队列';
  } else if (isDraining) {
    action = 'draining';
    label = '收尾中...';
    disabled = true;
  }

  return {
    action,
    label,
    disabled,
    isDraining,
  };
}

export async function toggleQueue(action, invokeFn = defaultInvoke) {
  if (action === 'pause') {
    await invokeFn('pause_queue');
    return;
  }

  if (action === 'start' || action === 'resume') {
    await invokeFn('start_queue');
  }
}

export async function runQueueToggle({
  disabled,
  action,
  setBusy,
  reloadQueueState,
  invokeFn = defaultInvoke,
  onError = console.error,
}) {
  if (disabled) return;

  setBusy(true);
  try {
    await toggleQueue(action, invokeFn);
    await reloadQueueState();
  } catch (error) {
    onError(error);
  } finally {
    setBusy(false);
  }
}
