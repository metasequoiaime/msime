const candidatePageSizes = Array.from({ length: 7 }, (_, index) => index + 3);

export function offeredCandidatePageSizes(current: number): number[] {
  return candidatePageSizes.includes(current)
    ? candidatePageSizes
    : [current, ...candidatePageSizes];
}
