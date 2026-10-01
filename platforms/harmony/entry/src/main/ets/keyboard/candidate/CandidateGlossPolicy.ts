/**
 * Lifecycle and presentation rules for optional offline candidate glosses, ported from
 * platforms/android/java/app/msime/android/CandidateGlossPolicy.java.
 *
 * A gloss arrives asynchronously, so the token says which session, generation and epoch asked for it.
 * Anything stale is dropped rather than painted onto whatever is on screen now.
 */
import { utf8Length } from "../Utf8";

const MAX_ENTRY_BYTES: number = 4096;

export interface GlossToken {
  readonly session: number;
  readonly generation: number;
  readonly epoch: number;
}

export class CandidateGlossPolicy {
  static token(session: number, generation: number, epoch: number): GlossToken {
    if (session <= 0 || generation < 0 || epoch < 0) {
      throw new Error("Invalid candidate gloss token");
    }
    return { session: session, generation: generation, epoch: epoch };
  }

  static isCurrent(
    token: GlossToken,
    currentSession: number,
    currentGeneration: number,
    currentEpoch: number,
  ): boolean {
    return (
      token.session === currentSession &&
      token.generation === currentGeneration &&
      token.epoch === currentEpoch
    );
  }

  /** Engine annotations, for example Wubi codes, occupy the shared hint slot first. */
  /**
   * Whether the annotation slot is carrying a translation rather than an Engine annotation.
   *
   * The two share a slot here but not in the source, which styles the translation in its own right
   * — `.cand-translation { font-size: 0.78em; opacity: 0.62 }`, identical in all four skins — and
   * says nothing at all about an Engine annotation. Keeping them apart is what lets the one with a
   * stated appearance take it without the one without a stated appearance having one invented.
   */
  static annotationIsTranslation(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): boolean {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return false;
    }
    return (
      glossEnabled &&
      CandidateGlossPolicy.bounded(translation) &&
      translation !== null &&
      translation.length > 0
    );
  }

  static annotation(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): string {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return engineAnnotation;
    }
    return glossEnabled && CandidateGlossPolicy.bounded(translation) && translation !== null
      ? translation
      : "";
  }

  static accessibilitySuffix(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): string {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return "，提示：" + engineAnnotation;
    }
    const gloss: string = CandidateGlossPolicy.annotation(
      engineAnnotation,
      translation,
      glossEnabled,
    );
    return gloss.length === 0 ? "" : "，英文释义：" + gloss;
  }

  /**
   * The 훈음 of a Korean Hanja row, which the Engine sends as the row's annotation (나라 이름 한 for 韓), or "" for any other row.
   *
   * A Korean user picks a Hanja by its 훈음, so it is drawn on the gloss line under the Hanja whatever the translation switches say, and the main text stays the Hanja alone. It is a separate field rather than the shared annotation slot because that slot is what the gloss menu types into the document, and the 훈음 is never committed.
   */
  static hunEum(koreanHanja: boolean, engineAnnotation: string | null): string {
    return koreanHanja && CandidateGlossPolicy.bounded(engineAnnotation) && engineAnnotation !== null
      ? engineAnnotation
      : "";
  }

  /** The Engine annotation left for the shared slot: none for a Korean Hanja row, whose annotation is its 훈음 and has a line of its own, so a translation can take the slot. */
  static slotAnnotation(koreanHanja: boolean, engineAnnotation: string | null): string | null {
    return koreanHanja ? null : engineAnnotation;
  }

  /** What the gloss line under a candidate says: the 훈음 and then the translation, on one line separated by a middle dot, either alone when the other is missing. */
  static glossLine(hunEum: string, translation: string): string {
    if (hunEum.length === 0) {
      return translation;
    }
    return translation.length === 0 ? hunEum : hunEum + " · " + translation;
  }

  /** The screen reader's words for a 훈음, read ahead of any translation suffix. */
  static hunEumAccessibilitySuffix(hunEum: string): string {
    return hunEum.length === 0 ? "" : "，训音：" + hunEum;
  }

  private static bounded(value: string | null): boolean {
    return value !== null && value.length > 0 && utf8Length(value) <= MAX_ENTRY_BYTES;
  }
}
