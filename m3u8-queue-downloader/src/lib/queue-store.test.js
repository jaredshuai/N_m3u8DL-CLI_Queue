import test from 'node:test';
import assert from 'node:assert/strict';
import { createQueueStateLoader, runStopTask } from './queue-store.js';

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

test('createQueueStateLoader serializes rapid reloads and applies only the latest snapshot', async () => {
  const first = deferred();
  const second = deferred();
  const appliedTasks = [];
  const appliedRunning = [];
  let invokeCount = 0;
  const loader = createQueueStateLoader({
    invokeQueueState: (() => {
      const responses = [first.promise, second.promise];
      return () => {
        invokeCount += 1;
        return responses.shift();
      };
    })(),
    setTasks: (tasks) => appliedTasks.push(tasks.map((task) => task.id)),
    setQueueRunning: (running) => appliedRunning.push(running),
    onError: () => {},
  });

  const firstLoad = loader();
  const secondLoad = loader();
  assert.equal(invokeCount, 1);

  first.resolve({
    tasks: [{ id: 'older', progress: 0.1 }],
    isRunning: false,
  });
  await Promise.resolve();
  assert.equal(invokeCount, 2);
  assert.deepEqual(appliedTasks, []);

  second.resolve({
    tasks: [{ id: 'newer', progress: 0.2 }],
    isRunning: true,
  });

  await firstLoad;
  await secondLoad;

  assert.deepEqual(appliedTasks, [['newer']]);
  assert.deepEqual(appliedRunning, [true]);
});

test('runStopTask invokes the stop command before reloading queue state', async () => {
  const calls = [];

  await runStopTask('task-1', {
    invokeCommand: async (command, payload) => {
      calls.push(['invoke', command, payload]);
    },
    reloadQueueState: async () => {
      calls.push(['reload']);
    },
  });

  assert.deepEqual(calls, [
    ['invoke', 'stop_task', { taskId: 'task-1' }],
    ['reload'],
  ]);
});

test('runStopTask reloads queue state when the stop command fails', async () => {
  const calls = [];

  await assert.rejects(
    runStopTask('task-2', {
      invokeCommand: async () => {
        calls.push(['invoke']);
        throw new Error('stop failed');
      },
      reloadQueueState: async () => {
        calls.push(['reload']);
      },
    }),
    /stop failed/,
  );

  assert.deepEqual(calls, [['invoke'], ['reload']]);
});
