import test from 'node:test';
import assert from 'node:assert/strict';
import {
  beginTerminalStateLoad,
  buildTerminalView,
  capRenderedTerminalLines,
  closeCliConsole,
  createCliConsoleState,
  createTerminalLoadState,
  findCliConsoleTask,
  openCliConsole,
  resolveTerminalActiveLine,
  shouldApplyTerminalResponse,
  shouldReloadTerminalState,
  shouldStartTerminalStateLoad,
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

test('capRenderedTerminalLines keeps only the newest render window', () => {
  const lines = ['line-1', 'line-2', 'line-3', 'line-4'];

  assert.deepEqual(capRenderedTerminalLines(lines, 2), ['line-3', 'line-4']);
});

test('resolveTerminalActiveLine shows loaded active line when live field is still empty', () => {
  const activeLine = resolveTerminalActiveLine(
    { id: 'task-1', terminalActiveLine: '' },
    'Progress: 126/1095 (11.51%)'
  );

  assert.equal(activeLine, 'Progress: 126/1095 (11.51%)');
});

test('resolveTerminalActiveLine falls back when live field is absent', () => {
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

test('shouldApplyTerminalResponse accepts only the latest request token', () => {
  assert.equal(shouldApplyTerminalResponse(3, 3), true);
  assert.equal(shouldApplyTerminalResponse(2, 3), false);
});

test('terminal load state marks an in-flight task as already requested', () => {
  const task = { id: 'task-1', status: 'downloading' };
  let loadState = createTerminalLoadState();

  assert.equal(shouldStartTerminalStateLoad(task, loadState), true);

  loadState = beginTerminalStateLoad(loadState, task);

  assert.equal(shouldStartTerminalStateLoad(task, loadState), false);
  assert.equal(
    shouldStartTerminalStateLoad({ ...task, status: 'completed' }, loadState),
    true,
  );
  assert.equal(loadState.requestId, 1);
});
