/**
 * What the keyboard may do for a given editor, reworked from
 * platforms/android/java/app/msime/android/EditorPolicy.java.
 *
 * This one is not a transcription. Android describes an editor with an InputType bitmask — a class,
 * a variation and flags, all packed into one int — while HarmonyOS hands over a single inputPattern
 * enumeration plus its own capitalize mode. The decisions are the same; the way the question arrives
 * is not.
 *
 * The platform constants are deliberately absent here. PATTERN_TEXT and its siblings are declared in
 * @ohos.inputMethodEngine without values, so their numbers belong to the runtime and must not be
 * guessed. The caller translates inputPattern into these traits, which keeps this file plain
 * TypeScript and therefore testable without a device.
 */
import { CapitalizationMode } from "./EnglishCapitalizationPolicy";

export interface EditorTraits {
  /** The editor takes prose, as opposed to a number, phone number or date. */
  readonly text: boolean;
  readonly password: boolean;
  readonly uri: boolean;
  readonly email: boolean;
  /** The editor asked for no suggestions — a one-time code field, for instance. */
  readonly noSuggestions: boolean;
}

export class EditorPolicy {
  /** A late editor-attribute callback must not reset text typed while the query was in flight. */
  static appliesDelayedLanguage(
    composing: boolean,
    currentEnglish: boolean,
    wantedEnglish: boolean,
    manuallyChosen: boolean = false,
  ): boolean {
    return !composing && !manuallyChosen && currentEnglish !== wantedEnglish;
  }

  /**
   * Whether to compose through the Engine at all. A password field must never see a composition
   * buffer, and a field that asked for no suggestions has said it does not want one.
   */
  static useEngine(traits: EditorTraits): boolean {
    return traits.text && !traits.password && !traits.noSuggestions;
  }

  /**
   * Whether the key heatmap must not count presses in this editor: a password field only. A number, phone, date or no-suggestions field is still the user's typing and counts, as on Android (`EditorPolicy.excludesKeyStatistics`); HarmonyOS's editor attribute carries no incognito or no-learning signal to add to it.
   */
  static excludesKeyStatistics(traits: EditorTraits): boolean {
    return traits.password;
  }

  /** The keyboard opens in Latin for fields where Chinese input would only be in the way. */
  static prefersLatin(traits: EditorTraits): boolean {
    return traits.uri || traits.email || traits.password || traits.noSuggestions;
  }

  /**
   * Whether the letter face has to be the full twenty-six keys rather than a nine-key grid.
   *
   * The same editors as prefersLatin, for a different reason, so the question gets its own name: a
   * grid spells by disambiguating a digit sequence against a dictionary, and an address, a password
   * or a one-time code is arbitrary text that no dictionary contains. There every tap has to be one
   * unambiguous character. This is the editor's override rather than the user's, so it is applied on
   * attach and never written back to the layout preference.
   */
  static prefersFullFace(traits: EditorTraits): boolean {
    return EditorPolicy.prefersLatin(traits);
  }

  /**
   * HarmonyOS supplies a capitalize mode directly, where Android leaves it to be derived from
   * CAP_CHARACTERS, CAP_WORDS and CAP_SENTENCES flags. The platform value is honoured except where
   * the Java also overrode it: a URI or an email address is never capitalized, because the first
   * character is part of an address rather than a sentence.
   */
  static capitalizationMode(
    traits: EditorTraits,
    platformMode: CapitalizationMode,
  ): CapitalizationMode {
    if (!traits.text || traits.uri || traits.email) {
      return CapitalizationMode.NONE;
    }
    return platformMode;
  }
}
