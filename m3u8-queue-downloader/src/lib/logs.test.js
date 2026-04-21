import test from 'node:test';
import assert from 'node:assert/strict';
import { MAX_LOG_LINES, appendLogLine } from './logs.js';

test('appendLogLine keeps only the newest MAX_LOG_LINES entries', () => {
  let lines = [];

  for (let i = 0; i < MAX_LOG_LINES + 25; i += 1) {
    lines = appendLogLine(lines, `line-${i}`);
  }

  assert.equal(lines.length, MAX_LOG_LINES);
  assert.equal(lines[0], 'line-25');
  assert.equal(lines.at(-1), `line-${MAX_LOG_LINES + 24}`);
});
