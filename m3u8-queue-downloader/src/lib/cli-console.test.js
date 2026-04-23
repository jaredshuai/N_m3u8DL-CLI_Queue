import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildTerminalView,
  closeCliConsole,
  createCliConsoleState,
  findCliConsoleTask,
  openCliConsole,
  resolveTerminalActiveLine,
  shouldReloadTerminalState,
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

test('buildTerminalView separates committed lines from active line', () => {
  const view = buildTerminalView({
    terminalCommittedLines: ['Starting download', 'Connecting...'],
    terminalActiveLine: 'Progress: 50/100 (50.00%)',
  });

  assert.deepEqual(view.committedLines, ['Starting download', 'Connecting...']);
  assert.equal(view.activeLine, 'Progress: 50/100 (50.00%)');
});

test('buildTerminalView returns empty state for null task', () => {
  const view = buildTerminalView(null);

  assert.deepEqual(view.committedLines, []);
  assert.equal(view.activeLine, '');
});

test('buildTerminalView handles task with no active line', () => {
  const view = buildTerminalView({
    terminalCommittedLines: ['line-1', 'line-2'],
  });

  assert.deepEqual(view.committedLines, ['line-1', 'line-2']);
  assert.equal(view.activeLine, '');
});

test('resolveTerminalActiveLine prefers explicit empty live value over persisted fallback', () => {
  const activeLine = resolveTerminalActiveLine(
    { id: 'task-1', terminalActiveLine: '' },
    'Progress: 126/1095 (11.51%)'
  );

  assert.equal(activeLine, '');
});

test('resolveTerminalActiveLine falls back only when live field is absent', () => {
  const activeLine = resolveTerminalActiveLine(
    { id: 'task-1' },
    'Progress: 126/1095 (11.51%)'
  );

  assert.equal(activeLine, 'Progress: 126/1095 (11.51%)');
});

test('shouldReloadTerminalState reloads when task id changes', () => {
  assert.equal(
    shouldReloadTerminalState({ id: 'task-2', status: 'downloading' }, 'task-1', 'downloading'),
    true
  );
});

test('shouldReloadTerminalState reloads when same task changes status', () => {
  assert.equal(
    shouldReloadTerminalState({ id: 'task-1', status: 'completed' }, 'task-1', 'downloading'),
    true
  );
});

test('shouldReloadTerminalState skips reload when task id and status are unchanged', () => {
  assert.equal(
    shouldReloadTerminalState({ id: 'task-1', status: 'downloading' }, 'task-1', 'downloading'),
    false
  );
});
