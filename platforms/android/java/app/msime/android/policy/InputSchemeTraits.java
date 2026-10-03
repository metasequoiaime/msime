package app.msime.android;

/**
 * Scheme behaviour this host decides from a view's `scheme` number.
 *
 * <p>The Engine's `SchemeType` const fns (crates/engine/src/types.rs) are the source of truth; the view publishes only some of them, so each function below that is named after a predicate copies it, and scripts/test-scheme-traits-parity.py checks every one against the engine for every scheme. An unknown scheme number answers false everywhere, the way host-api reads `SchemeType::from_u8`.
 */
public final class InputSchemeTraits {
    // The Engine's `SchemeType` ordinals, as they appear in a view's `scheme`.
    public static final int QUANPIN = 0;
    public static final int SHUANGPIN = 1;
    public static final int WUBI = 2;
    public static final int JAPANESE = 3;
    public static final int KOREAN = 4;
    public static final int CANTONESE = 5;
    public static final int ZHUYIN = 6;
    public static final int VIETNAMESE = 7;
    public static final int STROKE = 8;

    private InputSchemeTraits() {}

    // `is_chinese`: a Chinese scheme, the kind `last_chinese_scheme` remembers.
    public static boolean isChinese(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == CANTONESE || scheme == ZHUYIN || scheme == STROKE; }

    // `outputs_traditional_natively`: the scheme's own candidates are Traditional characters, so there is nothing for Simplified-to-Traditional conversion to do.
    public static boolean outputsTraditionalNatively(int scheme) { return scheme == CANTONESE || scheme == ZHUYIN || scheme == STROKE; }

    // `script_conversion_applies`: the host's Simplified-to-Traditional output conversion runs on this scheme's commits.
    public static boolean scriptConversionApplies(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI; }

    // `uses_chinese_punctuation`: punctuation goes through the Chinese table. Korean and Vietnamese write half-width ASCII marks whatever the Chinese punctuation switch says.
    public static boolean usesChinesePunctuation(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == JAPANESE || scheme == CANTONESE || scheme == ZHUYIN || scheme == STROKE; }

    // `widens_full_width`: commits and direct characters are widened when the full-width switch is on.
    public static boolean widensFullWidth(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == JAPANESE || scheme == CANTONESE || scheme == ZHUYIN || scheme == STROKE; }

    // `opens_local_modes`: the local modes (`/` commands, `@` mentions, the tools that start them) are offered.
    public static boolean opensLocalModes(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN; }

    // `shows_glosses`: candidates may carry translation glosses.
    public static boolean showsGlosses(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == KOREAN; }

    // `commits_on_blur`: leaving the composition (focus loss, a caret jump, a scheme or mode switch) writes it out instead of discarding it.
    public static boolean commitsOnBlur(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE; }

    // `draws_reading`: the composition to mark inline is the view's `reading`, not its `editing_text`.
    public static boolean drawsReading(int scheme) { return scheme == JAPANESE || scheme == KOREAN || scheme == ZHUYIN || scheme == STROKE; }

    // `has_openable_candidate_list`: candidates appear only in a list the user opens with command 16 (the Korean Hanja list, the Zhuyin list).
    public static boolean hasOpenableCandidateList(int scheme) { return scheme == KOREAN || scheme == ZHUYIN; }

    // `cancel_keeps_composition`: the first Cancel keeps the composition (it closes the open candidate list, or takes a Vietnamese word back to its raw keys), so discarding takes a second one.
    public static boolean cancelKeepsComposition(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE; }

    // `locks_caret`: the caret stays at the end of the composition, so the caret keys write it out and keep their meaning in the editor.
    public static boolean locksCaret(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE; }

    // Host-only: the scheme number is one of the nine above. A gate that existed before a scheme did keeps its old answer for a number it does not know rather than the false every trait gives it.
    public static boolean known(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == JAPANESE || scheme == KOREAN || scheme == CANTONESE || scheme == ZHUYIN || scheme == VIETNAMESE || scheme == STROKE; }

    // Host-only: letters build the written text directly (a Hangul syllable, a Vietnamese word), so Shift is the letter's case rather than the language switch and there is no word to take a character from.
    public static boolean letterComposition(int scheme) { return scheme == KOREAN || scheme == VIETNAMESE; }
}
