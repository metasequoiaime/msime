/**
 * Shared preference gates for the two optional annotations a candidate can carry.
 *
 * Both arrive in the prepared host document, where the shared store always writes them, and they differ in their defaults: `wubi_code_hint` is on unless the document says otherwise, `candidate_english_gloss` is off unless it says otherwise.
 */
export class CandidateAnnotationPreferencePolicy {
  /** Anything but an explicit false keeps the hint on, matching the shared default. */
  static wubiCodeHint(value: unknown): boolean {
    return value !== false;
  }

  /** Offline glosses are off unless the document says otherwise. */
  static englishGloss(value: unknown): boolean {
    return value === true;
  }
}
