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
    public static final int TIBETAN = 8;
    public static final int STROKE = 9;

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

    // `opens_local_modes`: Shift+letter opens the local modes other than K while nothing is composed (the pinyin schemes). The Engine opens K, `/` and `@` under `opens_table_modes`, which adds Wubi, but the toolbar entry for the local modes that this gates stays pinyin-only by choice (.agents/notes/implemented/bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md).
    public static boolean opensLocalModes(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN; }

    // `shows_glosses`: candidates may carry translation glosses.
    public static boolean showsGlosses(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == KOREAN; }

    // `commits_on_blur`: leaving the composition (focus loss, a caret jump, a scheme or mode switch) writes it out instead of discarding it.
    public static boolean commitsOnBlur(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE || scheme == TIBETAN; }

    // `draws_reading`: the composition to mark inline is the view's `reading`, not its `editing_text`.
    public static boolean drawsReading(int scheme) { return scheme == JAPANESE || scheme == KOREAN || scheme == ZHUYIN || scheme == STROKE; }

    // `has_openable_candidate_list`: candidates appear only in a list the user opens with command 16 (the Korean Hanja list, the Zhuyin list).
    public static boolean hasOpenableCandidateList(int scheme) { return scheme == KOREAN || scheme == ZHUYIN; }

    // `cancel_keeps_composition`: 第一次取消保留组字（关闭打开的候选列表，或把越南语单词、藏文音节退回原始按键），要丢弃组字需要再取消一次。
    public static boolean cancelKeepsComposition(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE || scheme == TIBETAN; }

    // `locks_caret`: the caret stays at the end of the composition, so the caret keys write it out and keep their meaning in the editor.
    public static boolean locksCaret(int scheme) { return scheme == KOREAN || scheme == ZHUYIN || scheme == VIETNAMESE || scheme == TIBETAN; }

    // 宿主专用：方案序号是上面十个之一。某个判断早于某个方案存在时，对不认识的序号沿用它原来的答案，而不是各谓词统一给出的 false。
    public static boolean known(int scheme) { return scheme == QUANPIN || scheme == SHUANGPIN || scheme == WUBI || scheme == JAPANESE || scheme == KOREAN || scheme == CANTONESE || scheme == ZHUYIN || scheme == VIETNAMESE || scheme == TIBETAN || scheme == STROKE; }

    // 宿主专用：字母直接拼成书写的文字（韩文音节、越南语单词、按区分大小写的威利转写拼出的藏文音节），所以 Shift 是字母大小写而不是语言切换，也没有可以以词定字的词。
    public static boolean letterComposition(int scheme) { return scheme == KOREAN || scheme == VIETNAMESE || scheme == TIBETAN; }
}
