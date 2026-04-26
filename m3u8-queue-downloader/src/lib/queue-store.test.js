import test from 'node:test';
import assert from 'node:assert/strict';
import { createQueueStateLoader } from './queue-store.js';

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

test('createQueueStateLoader applies only the latest resolved queue snapshot', async () => {
  const first = deferred();
  const second = deferred();
  const appliedTasks = [];
  const appliedRunning = [];
  const loader = createQueueStateLoader({
    invokeQueueState: (() => {
      const responses = [first.promise, second.promise];
      return () => responses.shift();
    })(),
    setTasks: (tasks) => appliedTasks.push(tasks.map((task) => task.id)),
    setQueueRunning: (running) => appliedRunning.push(running),
    onError: () => {},
  });

  const firstLoad = loader();
  const secondLoad = loader();

  second.resolve({
    tasks: [{ id: 'newer', progress: 0.2 }],
    isRunning: true,
  });
  await secondLoad;

  first.resolve({
    tasks: [{ id: 'older', progress: 0.1 }],
    isRunning: false,
  });
  await firstLoad;

  assert.deepEqual(appliedTasks, [['newer']]);
  assert.deepEqual(appliedRunning, [true]);
});
