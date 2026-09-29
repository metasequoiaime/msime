/**
 * Whether Japanese Space converts the composition or commits it, ported from platforms/android/java/app/msime/android/policy/JapaneseSpacePolicy.java.
 *
 * Space normally starts a conversion and later presses step through the candidates. A lone Fallback row is the raw composition the Engine shows when there is nothing to convert (a bare Shift+R prefix, or romaji it cannot read); Windows commits it on the first Space, so Space takes the normal commit path instead.
 */
export class JapaneseSpacePolicy {
  /** The Engine's `CandidateSource::Fallback`, as it appears in a view candidate's `source`. */
  static readonly CANDIDATE_SOURCE_FALLBACK: number = 9;

  /** True when Space should arm or step a conversion over `candidateCount` rows whose first row has `firstSource`. */
  static converts(candidateCount: number, firstSource: number): boolean {
    if (candidateCount <= 0) {
      return false;
    }
    return !(candidateCount === 1 && firstSource === JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK);
  }
}
