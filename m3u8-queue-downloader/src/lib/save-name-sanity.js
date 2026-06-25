function normalizeText(value) {
  return (value ?? '').trim();
}

/**
 * Detects the classic paste-failure pitfall: the user copied the m3u8 URL,
 * pasted it into the URL field, then tried to copy a save name but the copy
 * failed — so pasting into the save-name field yields the same URL string.
 * In that case saveName and url are identical, which is almost never the
 * user's intent. See ADR / sanity check feature.
 *
 * Returns a warning object when the two fields are equal (after trim) and
 * non-empty; otherwise null.
 */
export function detectSaveNameUrlCollision({ url = '', saveName = '' } = {}) {
  const normalizedUrl = normalizeText(url);
  const normalizedSaveName = normalizeText(saveName);

  if (!normalizedUrl || !normalizedSaveName) {
    return null;
  }

  if (normalizedUrl === normalizedSaveName) {
    return {
      code: 'save-name-equals-url',
      message: '保存名称与链接完全相同，疑似复制失败粘贴错误，请检查',
    };
  }

  return null;
}
