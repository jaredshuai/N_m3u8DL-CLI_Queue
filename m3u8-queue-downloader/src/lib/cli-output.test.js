import test from 'node:test';
import assert from 'node:assert/strict';
import {
  mergeCliOutputLines,
  prependCliOutputPage,
} from './cli-output.js';

test('mergeCliOutputLines removes overlapping tail lines', () => {
  const merged = mergeCliOutputLines(
    ['line-1', 'line-2', 'line-3'],
    ['line-3', 'line-4'],
  );

  assert.deepEqual(merged, ['line-1', 'line-2', 'line-3', 'line-4']);
});

test('mergeCliOutputLines appends non-overlapping live tail', () => {
  const merged = mergeCliOutputLines(
    ['line-1', 'line-2'],
    ['line-3', 'line-4'],
  );

  assert.deepEqual(merged, ['line-1', 'line-2', 'line-3', 'line-4']);
});

test('prependCliOutputPage prepends older page without duplicating boundary', () => {
  const merged = prependCliOutputPage(
    ['line-3', 'line-4', 'line-5'],
    { lines: ['line-1', 'line-2', 'line-3'] },
  );

  assert.deepEqual(merged, ['line-1', 'line-2', 'line-3', 'line-4', 'line-5']);
});
