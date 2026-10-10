/**
 * The code the settings page decodes a refused preferences write into.
 *
 * The shared settings UI writes one Chinese sentence per failure and picks it by `error.code`. The
 * desktop host produces those codes by matching on `PreferencesError` before the failure ever
 * leaves Rust. This host reaches the same store through the C ABI, whose error channel is a single
 * string shared by every entry point, so what arrives here is the variant's English `Display` text
 * and the page had nothing to match on: a save refused because the keyboard wrote first showed the
 * sentence "preferences changed; reload before saving" to the user instead of asking them to reload.
 *
 * Mapping the text back is a weaker join than the desktop's, so it is deliberately narrow: only the
 * variants the shared UI actually has a sentence for are named, and anything else becomes `storage`,
 * which is the same code the desktop host uses for everything it does not name and which the page
 * answers with its general "cannot reach the settings" wording. A variant whose text drifts
 * therefore loses its specific sentence rather than producing a wrong one.
 *
 * The prefix checks are for the two variants that carry a nested cause in their text.
 */

export class PreferencesErrorCode {
  /**
   * The desktop's code for a refusal, or `storage` when the text is not one this names.
   *
   * An empty or missing text is `storage` as well: a refusal without a reason is still a refusal,
   * and the page should say the general thing rather than nothing at all.
   */
  static of(error: string): string {
    if (typeof error !== "string" || error.length === 0) {
      return "storage";
    }
    switch (error) {
      case "preferences changed; reload before saving":
        return "conflict";
      case "candidate page size must be between 1 and 10":
        return "invalid";
      case "frequency trigger count and linear step must be between 1 and 10":
        return "frequency_invalid";
      case "mixed English minimum prefix must be between 1 and 8":
        return "mixed_input_invalid";
      case "floating toolbar settings are invalid":
        return "floating_toolbar_invalid";
      case "word-to-character and paging cannot use the same keys":
        return "key_conflict";
      case "unsupported preferences format":
        return "format";
      default:
        break;
    }
    // `PreferencesError::Json` and the C ABI's own snapshot check both mean the document could not
    // be read, which is what the desktop reports as `format`.
    if (
      error.startsWith("invalid preferences document:") ||
      error === "invalid preferences snapshot"
    ) {
      return "format";
    }
    return "storage";
  }

  /**
   * The same reply with its error replaced by a code, left untouched when it is not a refusal.
   *
   * Parsing is defensive on purpose: this sits on the path that carries the whole document, and a
   * reply this cannot read is better forwarded as it is than replaced with a failure of its own.
   */
  static rewrite(reply: string): string {
    let parsed: PreferencesReply;
    try {
      parsed = JSON.parse(reply) as PreferencesReply;
    } catch {
      return reply;
    }
    if (parsed === null || typeof parsed !== "object" || parsed.ok !== false) {
      return reply;
    }
    return JSON.stringify({ ok: false, error: PreferencesErrorCode.of(parsed.error) });
  }
}

interface PreferencesReply {
  ok: boolean;
  error: string;
}
