export const CLI_OUTPUT_PAGE_SIZE = 200;

export function mergeCliOutputLines(persistedLines = [], tailLines = []) {
  const persisted = Array.isArray(persistedLines) ? persistedLines : [];
  const tail = Array.isArray(tailLines) ? tailLines : [];

  if (persisted.length === 0) return tail;
  if (tail.length === 0) return persisted;

  const maxOverlap = Math.min(persisted.length, tail.length);
  for (let overlap = maxOverlap; overlap > 0; overlap -= 1) {
    const persistedSuffix = persisted.slice(persisted.length - overlap);
    const tailPrefix = tail.slice(0, overlap);
    if (arraysEqual(persistedSuffix, tailPrefix)) {
      return [...persisted, ...tail.slice(overlap)];
    }
  }

  return [...persisted, ...tail];
}

export function prependCliOutputPage(currentLines = [], page) {
  return mergeCliOutputLines(page?.lines ?? [], currentLines);
}

function arraysEqual(a, b) {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}
