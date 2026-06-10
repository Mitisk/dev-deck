// Переместить элемент dragId так, чтобы он встал ПЕРЕД beforeId
// (beforeId === null или не найден → в конец). Возвращает новый массив id.
export function reorderIds(ids: number[], dragId: number, beforeId: number | null): number[] {
  if (beforeId === dragId) return [...ids]; // бросок перед самим собой — без изменений
  const without = ids.filter((id) => id !== dragId);
  if (beforeId === null) return [...without, dragId];
  const idx = without.indexOf(beforeId);
  if (idx < 0) return [...without, dragId];
  return [...without.slice(0, idx), dragId, ...without.slice(idx)];
}
