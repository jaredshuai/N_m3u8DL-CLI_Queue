import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createHistoryState,
  DEFAULT_HISTORY_PAGE_SIZE,
  mergeHistoryPage,
  prependHistoryTask,
  removeHistoryTask,
} from './history.js';

test('mergeHistoryPage replaces tasks on reset', () => {
  const state = mergeHistoryPage(createHistoryState(), {
    tasks: [{ id: 'a' }, { id: 'b' }],
    hasMore: true,
    nextOffset: 2,
  }, true);

  assert.deepEqual(state.tasks.map((task) => task.id), ['a', 'b']);
  assert.equal(state.hasMore, true);
  assert.equal(state.nextOffset, 2);
});

test('mergeHistoryPage appends tasks on load more', () => {
  const initial = {
    tasks: [{ id: 'a' }],
    hasMore: true,
    nextOffset: 1,
  };
  const state = mergeHistoryPage(initial, {
    tasks: [{ id: 'b' }, { id: 'c' }],
    hasMore: false,
    nextOffset: 3,
  });

  assert.deepEqual(state.tasks.map((task) => task.id), ['a', 'b', 'c']);
  assert.equal(state.hasMore, false);
  assert.equal(state.nextOffset, 3);
});

test('mergeHistoryPage deduplicates repeated tasks when concurrent pages overlap', () => {
  const initial = {
    tasks: [{ id: 'a' }, { id: 'b' }],
    hasMore: true,
    nextOffset: 2,
  };
  const state = mergeHistoryPage(initial, {
    tasks: [{ id: 'b' }, { id: 'c' }],
    hasMore: false,
    nextOffset: 4,
  });

  assert.deepEqual(state.tasks.map((task) => task.id), ['a', 'b', 'c']);
});

test('mergeHistoryPage preserves realtime tasks during initial reset', () => {
  const initial = {
    tasks: [{ id: 'live' }],
    hasMore: false,
    nextOffset: 1,
  };
  const state = mergeHistoryPage(initial, {
    tasks: [{ id: 'old-a' }, { id: 'old-b' }],
    hasMore: false,
    nextOffset: 2,
  }, true);

  assert.deepEqual(state.tasks.map((task) => task.id), ['live', 'old-a', 'old-b']);
  assert.equal(state.nextOffset, 2);
});

test('prependHistoryTask keeps the current window size', () => {
  const tasks = Array.from({ length: DEFAULT_HISTORY_PAGE_SIZE }, (_, index) => ({
    id: `task-${index}`,
  }));
  const state = prependHistoryTask({
    tasks,
    hasMore: false,
    nextOffset: DEFAULT_HISTORY_PAGE_SIZE,
  }, { id: 'new-task' });

  assert.equal(state.tasks.length, DEFAULT_HISTORY_PAGE_SIZE);
  assert.equal(state.tasks[0].id, 'new-task');
  assert.equal(state.tasks.at(-1).id, 'task-18');
  assert.equal(state.hasMore, true);
});

test('prependHistoryTask keeps growing until the default page size is reached', () => {
  let state = createHistoryState();

  state = prependHistoryTask(state, { id: 'task-a' });
  state = prependHistoryTask(state, { id: 'task-b' });

  assert.deepEqual(state.tasks.map((task) => task.id), ['task-b', 'task-a']);
});

test('prependHistoryTask updates nextOffset until the visible window is full', () => {
  let state = {
    tasks: [{ id: 'task-5' }, { id: 'task-4' }, { id: 'task-3' }, { id: 'task-2' }, { id: 'task-1' }],
    hasMore: false,
    nextOffset: 5,
  };

  for (let index = 6; index <= 20; index += 1) {
    state = prependHistoryTask(state, { id: `task-${index}` });
  }

  assert.equal(state.tasks.length, 20);
  assert.equal(state.nextOffset, 20);
  assert.equal(state.hasMore, false);

  state = prependHistoryTask(state, { id: 'task-21' });
  assert.equal(state.tasks.length, 20);
  assert.equal(state.nextOffset, 20);
  assert.equal(state.hasMore, true);
});

test('removeHistoryTask removes a visible task and decrements nextOffset', () => {
  const state = removeHistoryTask({
    tasks: [{ id: 'c' }, { id: 'b' }, { id: 'a' }],
    hasMore: true,
    nextOffset: 3,
  }, 'b');

  assert.deepEqual(state.tasks.map((task) => task.id), ['c', 'a']);
  assert.equal(state.nextOffset, 2);
  assert.equal(state.hasMore, true);
});
