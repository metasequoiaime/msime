/** 宿主没报告上限时滑块给到的最大每页候选数，与引入可选上限之前相同。 */
export const DEFAULT_MAX_CANDIDATE_PAGE_SIZE = 9;

/** 滑块提供的每页候选数：3 到宿主的上限（`HostCapabilities.max_candidate_page_size`，macOS、Linux、Android 为 10）；存着的值不在这个范围里时把范围放宽到它，而不是改写它。 */
export function offeredCandidatePageSizes(
  current: number,
  max: number = DEFAULT_MAX_CANDIDATE_PAGE_SIZE,
): number[] {
  const sizes = Array.from({ length: Math.max(1, max - 2) }, (_, index) => index + 3);
  return sizes.includes(current) ? sizes : [current, ...sizes];
}
