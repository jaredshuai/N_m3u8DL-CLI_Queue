import test from 'node:test';
import assert from 'node:assert/strict';
import { getTaskIdSignature, shouldSyncDndItems, toDndItems } from './waiting-dnd.js';

test('toDndItems preserves task ids while cloning task objects', () => {
  const waitingTasks = [
    { id: 'task-1', url: 'https://example.com/1.m3u8' },
    { id: 'task-2', url: 'https://example.com/2.m3u8' },
  ];

  const items = toDndItems(waitingTasks);

  assert.deepEqual(items.map((item) => item.id), ['task-1', 'task-2']);
  assert.notEqual(items[0], waitingTasks[0]);
});

test('getTaskIdSignature returns stable id ordering', () => {
  assert.equal(
    getTaskIdSignature([{ id: 'task-1' }, { id: 'task-2' }, { id: 'task-3' }]),
    'task-1|task-2|task-3'
  );
});

test('shouldSyncDndItems skips sync while drag state is locked', () => {
  assert.equal(
    shouldSyncDndItems({
      waitingTasks: [{ id: 'task-1' }],
      dndItems: [],
      syncLocked: true,
    }),
    false
  );
});

test('shouldSyncDndItems skips sync when task id order is unchanged', () => {
  assert.equal(
    shouldSyncDndItems({
      waitingTasks: [{ id: 'task-1', progress: 0 }, { id: 'task-2', progress: 0 }],
      dndItems: [{ id: 'task-1' }, { id: 'task-2' }],
      syncLocked: false,
    }),
    false
  );
});

test('shouldSyncDndItems requests sync when waiting task order changes', () => {
  assert.equal(
    shouldSyncDndItems({
      waitingTasks: [{ id: 'task-2' }, { id: 'task-1' }],
      dndItems: [{ id: 'task-1' }, { id: 'task-2' }],
      syncLocked: false,
    }),
    true
  );
});
