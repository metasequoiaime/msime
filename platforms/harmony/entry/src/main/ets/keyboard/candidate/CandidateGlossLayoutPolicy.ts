export interface CandidateGlossProviderState {
  readonly customEnabled: boolean;
  readonly customEndpoint: string;
  readonly niuTransEnabled: boolean;
  readonly niuTransAppId: string;
  readonly niuTransApiKey: string;
  readonly tencentEnabled: boolean;
  readonly tencentSecretId: string;
  readonly tencentSecretKey: string;
}

/** Presentation state that is known before an asynchronous candidate gloss arrives. */
export class CandidateGlossLayoutPolicy {
  /** Online translations and the packaged English dictionary are independent user switches. */
  static displays(offlineEnglish: boolean, onlineTranslations: boolean): boolean {
    return offlineEnglish || onlineTranslations;
  }

  /**
   * Harmony merges multiple language answers into one bounded line. Reserve that line from the
   * setting and usable provider state, rather than growing a candidate when its answer arrives.
   */
  static rows(
    offlineEnglish: boolean,
    targetLanguages: string[],
    onlineTranslations: boolean,
    providers: CandidateGlossProviderState,
    offlineTargets: string[],
  ): number {
    // An installed non-English dictionary answers under either switch, as the shared translation query does.
    const offline: boolean =
      (offlineEnglish && targetLanguages.includes("en")) ||
      ((offlineEnglish || onlineTranslations) &&
        targetLanguages.some((target: string): boolean => offlineTargets.includes(target)));
    const online: boolean = onlineTranslations && CandidateGlossLayoutPolicy.hasProvider(providers);
    return offline || online ? 1 : 0;
  }

  /**
   * The rows the strip keeps under its candidates for the current scheme. Korean keeps at least one whatever the switches say, because its Hanja rows always draw their 훈음 there; it follows the scheme rather than whether the Hanja list is open, so opening the list never changes the keyboard's height.
   */
  static schemeRows(rows: number, korean: boolean): number {
    return korean ? Math.max(rows, 1) : rows;
  }

  private static hasProvider(providers: CandidateGlossProviderState): boolean {
    if (providers.niuTransEnabled) {
      return (
        CandidateGlossLayoutPolicy.usableCredential(providers.niuTransAppId) &&
        CandidateGlossLayoutPolicy.usableCredential(providers.niuTransApiKey)
      );
    }
    if (providers.customEnabled) {
      return providers.customEndpoint.trim().length > 0;
    }
    return (
      providers.tencentEnabled &&
      CandidateGlossLayoutPolicy.usableCredential(providers.tencentSecretId) &&
      CandidateGlossLayoutPolicy.usableCredential(providers.tencentSecretKey)
    );
  }

  /** Mirrors the shared provider guard without retaining or exposing the credential. */
  private static usableCredential(value: string): boolean {
    const trimmed: string = value.trim();
    return (
      trimmed.length > 0 &&
      !(trimmed.startsWith("<") && trimmed.endsWith(">")) &&
      !trimmed.startsWith("FAKESECRET_")
    );
  }
}
