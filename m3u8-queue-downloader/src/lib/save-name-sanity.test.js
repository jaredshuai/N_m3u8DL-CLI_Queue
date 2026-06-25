import test from 'node:test';
import assert from 'node:assert/strict';
import { detectSaveNameUrlCollision } from './save-name-sanity.js';

test('detectSaveNameUrlCollision flags identical url and saveName', () => {
  const result = detectSaveNameUrlCollision({
    url: 'https://example.com/foo.m3u8',
    saveName: 'https://example.com/foo.m3u8',
  });

  assert.equal(result.code, 'save-name-equals-url');
  assert.match(result.message, /完全相同/);
});

test('detectSaveNameUrlCollision trims whitespace before comparing', () => {
  const result = detectSaveNameUrlCollision({
    url: '  https://example.com/foo.m3u8  ',
    saveName: 'https://example.com/foo.m3u8\n',
  });

  assert.equal(result.code, 'save-name-equals-url');
});

test('detectSaveNameUrlCollision returns null for distinct values', () => {
  const result = detectSaveNameUrlCollision({
    url: 'https://example.com/foo.m3u8',
    saveName: 'my-movie',
  });

  assert.equal(result, null);
});

test('detectSaveNameUrlCollision returns null when saveName is empty', () => {
  const result = detectSaveNameUrlCollision({
    url: 'https://example.com/foo.m3u8',
    saveName: '   ',
  });

  assert.equal(result, null);
});

test('detectSaveNameUrlCollision returns null when url is empty', () => {
  const result = detectSaveNameUrlCollision({
    url: '',
    saveName: 'my-movie',
  });

  assert.equal(result, null);
});

test('detectSaveNameUrlCollision returns null for default empty args', () => {
  const result = detectSaveNameUrlCollision();

  assert.equal(result, null);
});
