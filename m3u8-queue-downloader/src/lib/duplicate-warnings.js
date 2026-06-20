function normalizeText(value) {
  return (value ?? '').trim();
}

function normalizeOptional(value) {
  const text = normalizeText(value);
  return text.length > 0 ? text : null;
}

export function findDuplicateWarnings({ tasks = [], url = '', saveName = '' } = {}) {
  const normalizedUrl = normalizeText(url);
  const normalizedSaveName = normalizeOptional(saveName);

  if (!normalizedUrl) {
    return [];
  }

  const warnings = [];
  const lastTask = tasks.length > 0 ? tasks.at(-1) : null;

  if (lastTask?.url?.trim() === normalizedUrl) {
    warnings.push({
      code: 'adjacent-url',
      message: '新任务链接与上一条任务相同',
    });
  }

  const lastSaveName = normalizeOptional(lastTask?.saveName ?? lastTask?.save_name);
  if (normalizedSaveName && lastSaveName === normalizedSaveName) {
    warnings.push({
      code: 'adjacent-save-name',
      message: '新任务保存名称与上一条任务相同',
    });
  }

  const hasExactDuplicate = tasks.some((task) => {
    const taskUrl = normalizeText(task.url);
    const taskSaveName = normalizeOptional(task.saveName ?? task.save_name);
    return taskUrl === normalizedUrl && taskSaveName === normalizedSaveName;
  });

  if (hasExactDuplicate) {
    warnings.push({
      code: 'exact-duplicate',
      message: '队列中已存在 URL 与保存名称都相同的任务',
    });
  }

  return warnings;
}
