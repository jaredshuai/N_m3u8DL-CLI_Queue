import test from 'node:test';
import assert from 'node:assert/strict';
import {
  closeCliConsole,
  createCliConsoleState,
  findCliConsoleTask,
  openCliConsole,
} from './cli-console.js';

test('openCliConsole opens the panel for a task id', () => {
  const state = openCliConsole(createCliConsoleState(), 'task-1');

  assert.deepEqual(state, {
    open: true,
    taskId: 'task-1',
  });
});

test('closeCliConsole clears the selected task', () => {
  const state = closeCliConsole({
    open: true,
    taskId: 'task-1',
  });

  assert.deepEqual(state, {
    open: false,
    taskId: null,
  });
});

test('findCliConsoleTask resolves a task from active and historical lists', () => {
  const state = openCliConsole(createCliConsoleState(), 'task-2');
  const task = findCliConsoleTask(state, {
    tasks: [{ id: 'task-1' }],
    completedTasks: [{ id: 'task-2' }],
    failedTasks: [{ id: 'task-3' }],
  });

  assert.deepEqual(task, { id: 'task-2' });
});
