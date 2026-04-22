import test from 'node:test';
import assert from 'node:assert/strict';
import { findDuplicateWarnings } from './duplicate-warnings.js';

test('findDuplicateWarnings reports adjacent duplicate url without blocking', () => {
  const warnings = findDuplicateWarnings({
    tasks: [{ id: '1', url: 'https://example.com/a.m3u8', saveName: 'a' }],
    url: ' https://example.com/a.m3u8 ',
    saveName: 'b',
  });

  assert.ok(warnings.some((warning) => warning.code === 'adjacent-url'));
});

test('findDuplicateWarnings reports adjacent duplicate save name when provided', () => {
  const warnings = findDuplicateWarnings({
    tasks: [{ id: '1', url: 'https://example.com/a.m3u8', saveName: 'movie' }],
    url: 'https://example.com/b.m3u8',
    saveName: ' movie ',
  });

  assert.ok(warnings.some((warning) => warning.code === 'adjacent-save-name'));
});

test('findDuplicateWarnings reports exact duplicate task across the queue', () => {
  const warnings = findDuplicateWarnings({
    tasks: [
      { id: '1', url: 'https://example.com/a.m3u8', saveName: 'a' },
      { id: '2', url: 'https://example.com/b.m3u8', saveName: 'movie' },
    ],
    url: 'https://example.com/a.m3u8',
    saveName: 'a',
  });

  assert.ok(warnings.some((warning) => warning.code === 'exact-duplicate'));
});

test('findDuplicateWarnings ignores empty save name for adjacent save-name warnings', () => {
  const warnings = findDuplicateWarnings({
    tasks: [{ id: '1', url: 'https://example.com/a.m3u8', saveName: null }],
    url: 'https://example.com/b.m3u8',
    saveName: '',
  });

  assert.equal(warnings.some((warning) => warning.code === 'adjacent-save-name'), false);
});
