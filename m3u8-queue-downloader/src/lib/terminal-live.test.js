import test from 'node:test';
import assert from 'node:assert/strict';
import {
  appendTerminalCommittedLineBatch,
  applyTerminalActiveLineBatch,
  createTerminalCommittedLineDispatcher,
  createTerminalActiveLineDispatcher,
} from './terminal-live.js';

test('applyTerminalActiveLineBatch keeps explicit empty active lines', () => {
  const next = applyTerminalActiveLineBatch(
    { 'task-1': 'Progress: 10%' },
    { 'task-1': '', 'task-2': 'Progress: 20%' },
  );

  assert.deepEqual(next, {
    'task-1': '',
    'task-2': 'Progress: 20%',
  });
});

test('createTerminalActiveLineDispatcher coalesces updates until flush', () => {
  let scheduled = null;
  const flushed = [];
  const dispatcher = createTerminalActiveLineDispatcher({
    delay: 150,
    schedule: (callback, delay) => {
      scheduled = { callback, delay };
      return 1;
    },
    cancel: () => {},
    onFlush: (batch) => flushed.push(batch),
  });

  dispatcher.queue('task-1', 'Progress: 10%');
  dispatcher.queue('task-1', 'Progress: 11%');

  assert.equal(scheduled.delay, 150);
  assert.deepEqual(flushed, []);

  scheduled.callback();

  assert.deepEqual(flushed, [{ 'task-1': 'Progress: 11%' }]);
});

test('appendTerminalCommittedLineBatch appends per task without touching other tasks', () => {
  const next = appendTerminalCommittedLineBatch(
    {
      'task-1': ['line-1'],
      'task-2': ['other'],
    },
    {
      'task-1': ['line-2', 'line-3'],
    },
  );

  assert.deepEqual(next, {
    'task-1': ['line-1', 'line-2', 'line-3'],
    'task-2': ['other'],
  });
});

test('appendTerminalCommittedLineBatch caps retained lines per task', () => {
  const next = appendTerminalCommittedLineBatch(
    { 'task-1': ['line-1', 'line-2'] },
    { 'task-1': ['line-3', 'line-4'] },
    3,
  );

  assert.deepEqual(next, {
    'task-1': ['line-2', 'line-3', 'line-4'],
  });
});

test('createTerminalCommittedLineDispatcher batches committed lines until flush', () => {
  let scheduled = null;
  const flushed = [];
  const dispatcher = createTerminalCommittedLineDispatcher({
    delay: 150,
    schedule: (callback, delay) => {
      scheduled = { callback, delay };
      return 1;
    },
    cancel: () => {},
    onFlush: (batch) => flushed.push(batch),
  });

  dispatcher.queue('task-1', 'line-1');
  dispatcher.queue('task-1', 'line-2');

  assert.equal(scheduled.delay, 150);
  assert.deepEqual(flushed, []);

  scheduled.callback();

  assert.deepEqual(flushed, [{ 'task-1': ['line-1', 'line-2'] }]);
});
