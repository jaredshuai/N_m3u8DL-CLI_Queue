import { invoke } from '@tauri-apps/api/core';

export function getQueueControlState({ tasks, queueRunning, busy }) {
  const hasTasks = (tasks ?? []).length > 0;
  const action = queueRunning ? 'pause' : 'start';
  const label = queueRunning ? '暂停队列' : '开始队列';

  return {
    action,
    label,
    disabled: !hasTasks || busy,
  };
}

export async function toggleQueue(queueRunning, invokeFn = invoke) {
  await invokeFn(queueRunning ? 'pause_queue' : 'start_queue');
}

export async function runQueueToggle({
  disabled,
  queueRunning,
  setBusy,
  reloadQueueState,
  invokeFn = invoke,
  onError = console.error,
}) {
  if (disabled) return;

  setBusy(true);
  try {
    await toggleQueue(queueRunning, invokeFn);
    await reloadQueueState();
  } catch (error) {
    onError(error);
  } finally {
    setBusy(false);
  }
}
