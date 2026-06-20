import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createSessionProgressState,
  recordHistoricalSessionTask,
  trackSessionTask,
} from './session-progress.js';

test('trackSessionTask adds a task id once', () => {
  let state = createSessionProgressState();
  state = trackSessionTask(state, 'task-1');
  state = trackSessionTask(state, 'task-1');

  assert.deepEqual(state.pendingTaskIds, ['task-1']);
  assert.equal(state.completedCount, 0);
});

test('recordHistoricalSessionTask increments completed count for tracked completed tasks', () => {
  let state = createSessionProgressState();
  state = trackSessionTask(state, 'task-1');
  state = recordHistoricalSessionTask(state, { id: 'task-1', status: 'completed' });

  assert.deepEqual(state.pendingTaskIds, []);
  assert.equal(state.completedCount, 1);
});

test('recordHistoricalSessionTask drops tracked failed tasks without incrementing', () => {
  let state = createSessionProgressState();
  state = trackSessionTask(state, 'task-1');
  state = recordHistoricalSessionTask(state, { id: 'task-1', status: 'failed' });

  assert.deepEqual(state.pendingTaskIds, []);
  assert.equal(state.completedCount, 0);
});
