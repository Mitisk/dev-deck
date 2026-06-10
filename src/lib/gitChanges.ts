// Разбивка счётчиков git-статуса на категории для пилюли изменений.
// modified = всё грязное минус staged минус untracked (клампим в 0).
export type ChangeCategories = { modified: number; untracked: number; staged: number };

export function changeCategories(dirty: number, staged: number, untracked: number): ChangeCategories {
  return {
    modified: Math.max(0, dirty - staged - untracked),
    untracked,
    staged,
  };
}
