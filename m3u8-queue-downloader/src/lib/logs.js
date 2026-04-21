export const MAX_LOG_LINES = 500;

export function appendLogLine(lines, line) {
  const current = lines ?? [];
  if (current.length >= MAX_LOG_LINES) {
    const next = current.slice(-(MAX_LOG_LINES - 1));
    next.push(line);
    return next;
  }

  return [...current, line];
}
