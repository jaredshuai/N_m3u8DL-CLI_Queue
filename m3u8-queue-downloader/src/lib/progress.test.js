import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildProgressPatch,
  displayProgressPercent,
  normalizeBackendProgress,
  normalizeTaskProgress,
} from './progress.js';

test('normalizeBackendProgress converts backend ratio to frontend percent', () => {
  assert.equal(normalizeBackendProgress(0.345), 34.5);
  assert.equal(normalizeBackendProgress(1), 100);
});

test('normalizeBackendProgress returns null for missing progress instead of zero', () => {
  assert.equal(normalizeBackendProgress(-1), null);
  assert.equal(normalizeBackendProgress(null), null);
});

test('buildProgressPatch does not reset existing progress when payload has no progress', () => {
  assert.deepEqual(buildProgressPatch({ progress: -1, speed: '' }), {});
  assert.deepEqual(buildProgressPatch({ progress: null, speed: '2 MB/s' }), { speed: '2 MB/s' });
});

test('normalizeTaskProgress defaults only full task loads to zero', () => {
  assert.equal(normalizeTaskProgress({ id: '1', progress: -1 }).progress, 0);
});

test('displayProgressPercent clamps display value', () => {
  assert.equal(displayProgressPercent(12.4), 12);
  assert.equal(displayProgressPercent(101), 100);
  assert.equal(displayProgressPercent(-10), 0);
});
