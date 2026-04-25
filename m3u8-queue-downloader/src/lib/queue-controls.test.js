import test from 'node:test';
import assert from 'node:assert/strict';
import { getQueueControlState, runQueueToggle, toggleQueue } from './queue-controls.js';

test('getQueueControlState disables control when queue is empty', () => {
  const state = getQueueControlState({
    tasks: [],
    queueRunning: false,
    busy: false,
  });

  assert.equal(state.disabled, true);
  assert.equal(state.label, '开始队列');
  assert.equal(state.action, 'start');
});

test('getQueueControlState shows pause-next while a task is actively downloading', () => {
  const state = getQueueControlState({
    tasks: [{ id: '1', status: 'downloading' }, { id: '2', status: 'waiting' }],
    queueRunning: true,
    busy: false,
  });

  assert.equal(state.disabled, false);
  assert.equal(state.label, '暂停后续任务');
  assert.equal(state.action, 'pause');
});

test('getQueueControlState shows resume while queue is draining with waiting tasks left', () => {
  const state = getQueueControlState({
    tasks: [{ id: '1', status: 'downloading' }, { id: '2', status: 'waiting' }],
    queueRunning: false,
    busy: false,
  });

  assert.equal(state.disabled, false);
  assert.equal(state.label, '恢复队列');
  assert.equal(state.action, 'resume');
  assert.equal(state.isDraining, true);
});

test('getQueueControlState disables control while last task is draining', () => {
  const state = getQueueControlState({
    tasks: [{ id: '1', status: 'downloading' }],
    queueRunning: false,
    busy: false,
  });

  assert.equal(state.disabled, true);
  assert.equal(state.label, '收尾中...');
  assert.equal(state.action, 'draining');
});

test('getQueueControlState disables control while command is busy', () => {
  const state = getQueueControlState({
    tasks: [{ id: '1' }],
    queueRunning: false,
    busy: true,
  });

  assert.equal(state.disabled, true);
  assert.equal(state.label, '开始队列');
  assert.equal(state.action, 'start');
});

test('toggleQueue invokes start_queue when queue is stopped', async () => {
  const calls = [];

  await toggleQueue('start', async (command) => {
    calls.push(command);
  });

  assert.deepEqual(calls, ['start_queue']);
});

test('toggleQueue invokes pause_queue when queue is running', async () => {
  const calls = [];

  await toggleQueue('pause', async (command) => {
    calls.push(command);
  });

  assert.deepEqual(calls, ['pause_queue']);
});

test('toggleQueue invokes start_queue when queue is resumed from draining state', async () => {
  const calls = [];

  await toggleQueue('resume', async (command) => {
    calls.push(command);
  });

  assert.deepEqual(calls, ['start_queue']);
});

test('runQueueToggle flips busy state around a successful toggle and reload', async () => {
  const busyStates = [];
  const calls = [];

  await runQueueToggle({
    disabled: false,
    action: 'start',
    setBusy: (value) => busyStates.push(value),
    reloadQueueState: async () => calls.push('reload'),
    invokeFn: async (command) => calls.push(command),
    onError: () => calls.push('error'),
  });

  assert.deepEqual(calls, ['start_queue', 'reload']);
  assert.deepEqual(busyStates, [true, false]);
});

test('runQueueToggle clears busy state and reports error when toggle fails', async () => {
  const busyStates = [];
  const errors = [];

  await runQueueToggle({
    disabled: false,
    action: 'pause',
    setBusy: (value) => busyStates.push(value),
    reloadQueueState: async () => {
      throw new Error('reload should not run');
    },
    invokeFn: async () => {
      throw new Error('boom');
    },
    onError: (error) => errors.push(error.message),
  });

  assert.deepEqual(errors, ['boom']);
  assert.deepEqual(busyStates, [true, false]);
});

test('runQueueToggle does nothing when the control is disabled', async () => {
  const busyStates = [];
  const calls = [];

  await runQueueToggle({
    disabled: true,
    action: 'start',
    setBusy: (value) => busyStates.push(value),
    reloadQueueState: async () => calls.push('reload'),
    invokeFn: async (command) => calls.push(command),
    onError: () => calls.push('error'),
  });

  assert.deepEqual(calls, []);
  assert.deepEqual(busyStates, []);
});
