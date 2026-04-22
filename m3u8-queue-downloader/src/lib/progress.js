export function normalizeBackendProgress(progress) {
  if (progress == null || Number.isNaN(Number(progress))) {
    return null;
  }

  const ratio = Number(progress);
  if (ratio < 0) {
    return null;
  }

  return Math.min(100, Math.max(0, ratio * 100));
}

export function normalizeTaskProgress(task) {
  const progress = normalizeBackendProgress(task?.progress);
  return {
    ...task,
    progress: progress ?? 0,
  };
}

export function buildProgressPatch(payload = {}) {
  const patch = {};
  const progress = normalizeBackendProgress(payload.progress);

  if (progress != null) {
    patch.progress = progress;
  }

  if (payload.speed) {
    patch.speed = payload.speed;
  }

  if (payload.threads) {
    patch.threads = payload.threads;
  }

  return patch;
}

export function displayProgressPercent(progress) {
  const value = Number(progress ?? 0);
  if (Number.isNaN(value)) {
    return 0;
  }

  return Math.round(Math.min(100, Math.max(0, value)));
}
