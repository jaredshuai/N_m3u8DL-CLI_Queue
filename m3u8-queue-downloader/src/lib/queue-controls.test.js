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

test('getQueueControlState shows pause while queue is running', () => {
  const state = getQueueControlState({
    tasks: [{ id: '1' }],
    queueRunning: true,
    busy: false,
  });

  assert.equal(state.disabled, false);
  assert.equal(state.label, '暂停队列');
  assert.equal(state.action, 'pause');
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

  await toggleQueue(false, async (command) => {
    calls.push(command);
  });

  assert.deepEqual(calls, ['start_queue']);
});

test('toggleQueue invokes pause_queue when queue is running', async () => {
  const calls = [];

  await toggleQueue(true, async (command) => {
    calls.push(command);
  });

  assert.deepEqual(calls, ['pause_queue']);
});

test('runQueueToggle flips busy state around a successful toggle and reload', async () => {
  const busyStates = [];
  const calls = [];

  await runQueueToggle({
    disabled: false,
    queueRunning: false,
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
    queueRunning: true,
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
    queueRunning: false,
    setBusy: (value) => busyStates.push(value),
    reloadQueueState: async () => calls.push('reload'),
    invokeFn: async (command) => calls.push(command),
    onError: () => calls.push('error'),
  });

  assert.deepEqual(calls, []);
  assert.deepEqual(busyStates, []);
});
