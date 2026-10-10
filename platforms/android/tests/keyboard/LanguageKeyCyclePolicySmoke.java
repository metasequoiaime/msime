import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;
import app.msime.android.LanguageKeyCyclePolicy;
import app.msime.android.LanguageKeyCyclePolicy.Kind;
import app.msime.android.LanguageKeyCyclePolicy.Target;
import java.util.List;

public final class LanguageKeyCyclePolicySmoke {
    static final AppEdition FULL = AppEdition.FULL;

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    private static boolean is(Target target, Kind kind, KeyboardScheme scheme) {
        return target.kind() == kind && target.scheme() == scheme;
    }

    public static void main(String[] args) {
        List<KeyboardScheme> withOthers = List.of(KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.QUANPIN,
            KeyboardScheme.JAPANESE, KeyboardScheme.KOREAN);
        List<KeyboardScheme> chineseOnly = List.of(KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.HANDWRITING,
            KeyboardScheme.CANTONESE);
        KeyboardScheme back = KeyboardScheme.QUANPIN_NINE_KEY;

        // 开关关着：和原来一样只切中英，日语里点它也是去英文。
        check(is(LanguageKeyCyclePolicy.next(false, back, false, withOthers, back), Kind.ENGLISH, null), "off: 中 → 英");
        check(is(LanguageKeyCyclePolicy.next(false, back, true, withOthers, back), Kind.CHINESE_TOGGLE, null), "off: 英 → 中");
        check(is(LanguageKeyCyclePolicy.next(false, KeyboardScheme.JAPANESE, false, withOthers, back), Kind.ENGLISH, null),
            "off: Japanese still goes to English");
        check(LanguageKeyCyclePolicy.label(false, KeyboardScheme.JAPANESE, false, withOthers).equals("中"),
            "off: the key keeps its old face");

        // 开着但没有添加其他语言：仍只切中英。粤拼、手写是中文入口，不算其他语言。
        check(is(LanguageKeyCyclePolicy.next(true, back, false, chineseOnly, back), Kind.ENGLISH, null), "no others: 中 → 英");
        check(is(LanguageKeyCyclePolicy.next(true, back, true, chineseOnly, back), Kind.CHINESE_TOGGLE, null), "no others: 英 → 中");

        // 开着：中 → 英 → 日 → 韩 → 中（回到进入其他语言前的中文入口）。
        check(is(LanguageKeyCyclePolicy.next(true, back, false, withOthers, back), Kind.ENGLISH, null), "中 → 英");
        check(is(LanguageKeyCyclePolicy.next(true, back, true, withOthers, back), Kind.SCHEME, KeyboardScheme.JAPANESE),
            "英 → first other language in enabled order");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE, false, withOthers, back), Kind.SCHEME,
            KeyboardScheme.KOREAN), "日 → 韩");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.KOREAN, false, withOthers, back), Kind.SCHEME, back),
            "韩 → back to the Chinese entry");
        // 在日语里切到英文（Shift 或实体键盘快捷键）后，英文的下一步仍是第一种其他语言，再往后总能回到中文。
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE, true, withOthers, back), Kind.SCHEME,
            KeyboardScheme.JAPANESE), "English entered from Japanese continues the ring");

        // 方案偏好正在异步保存时，中英键不能把尚未应用的中文状态再切到英文。
        check(is(LanguageKeyCyclePolicy.next(true, true, back, false, withOthers, back), Kind.BUSY, null),
            "scheme save blocks a second language-key tap");

        // 按语言轮换：日语 9 键和 26 键都启用时只轮到一个，和中文入口同一种键盘的优先。
        List<KeyboardScheme> bothJapanese = List.of(KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY,
            KeyboardScheme.JAPANESE_NINE_KEY, KeyboardScheme.JAPANESE, KeyboardScheme.KOREAN);
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.QUANPIN_NINE_KEY, true, bothJapanese,
            KeyboardScheme.QUANPIN_NINE_KEY), Kind.SCHEME, KeyboardScheme.JAPANESE_NINE_KEY),
            "a nine-key Chinese user goes to Japanese nine-key");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.QUANPIN, true, bothJapanese, KeyboardScheme.QUANPIN),
            Kind.SCHEME, KeyboardScheme.JAPANESE), "a 26-key Chinese user goes to Japanese 26-key");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE_NINE_KEY, false, bothJapanese,
            KeyboardScheme.QUANPIN_NINE_KEY), Kind.SCHEME, KeyboardScheme.KOREAN),
            "Japanese is one stop, then Korean");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE, false, bothJapanese,
            KeyboardScheme.QUANPIN_NINE_KEY), Kind.SCHEME, KeyboardScheme.KOREAN),
            "either Japanese layout counts as the Japanese stop");
        check(is(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE_NINE_KEY, false,
            List.of(KeyboardScheme.QUANPIN, KeyboardScheme.JAPANESE_NINE_KEY), KeyboardScheme.QUANPIN), Kind.SCHEME,
            KeyboardScheme.QUANPIN), "the only Japanese layout is used even if it does not match");

        // 键面与读屏。
        check(LanguageKeyCyclePolicy.label(true, back, false, withOthers).equals("中"), "Chinese face");
        check(LanguageKeyCyclePolicy.label(true, back, true, withOthers).equals("英"), "English face");
        check(LanguageKeyCyclePolicy.label(true, KeyboardScheme.KOREAN, false, withOthers).equals("한"), "Korean face");
        check(LanguageKeyCyclePolicy.targetLabel(LanguageKeyCyclePolicy.next(true, KeyboardScheme.JAPANESE_NINE_KEY,
            false, List.of(back, KeyboardScheme.JAPANESE_NINE_KEY), back)).equals("中"),
            "the Japanese nine-key side key names Chinese when that is next");
        check(LanguageKeyCyclePolicy.targetLabel(LanguageKeyCyclePolicy.next(false, KeyboardScheme.JAPANESE_NINE_KEY,
            false, List.of(back, KeyboardScheme.JAPANESE_NINE_KEY), back)).equals("英"),
            "with the switch off the side key keeps 英");
        check(LanguageKeyCyclePolicy.description(new Target(Kind.ENGLISH, null)).equals("切换到英文输入"), "English description");
        check(LanguageKeyCyclePolicy.description(new Target(Kind.CHINESE_TOGGLE, null)).equals("切换到所选输入方案"),
            "Chinese description");
        check(LanguageKeyCyclePolicy.description(new Target(Kind.SCHEME, KeyboardScheme.KOREAN)).equals("切换到韩语 26 键"),
            "scheme description");
        check(LanguageKeyCyclePolicy.stateDescription(true, KeyboardScheme.JAPANESE, false, withOthers).equals("日语 26 键"),
            "state names the other language");

        // 回到中文的入口：最近用过的中文入口还在就用它，否则按 last_chinese_scheme，再否则第一个中文入口。
        check(LanguageKeyCyclePolicy.chineseReturn(KeyboardScheme.QUANPIN, "quanpin", withOthers, FULL,
            KeyboardScheme.QUANPIN) == KeyboardScheme.QUANPIN, "last used Chinese entry");
        check(LanguageKeyCyclePolicy.chineseReturn(null, "quanpin", withOthers, FULL, KeyboardScheme.QUANPIN)
            == KeyboardScheme.QUANPIN_NINE_KEY, "first entry of the stored Chinese scheme");
        check(LanguageKeyCyclePolicy.chineseReturn(KeyboardScheme.WUBI, "wubi", withOthers, FULL, KeyboardScheme.QUANPIN)
            == KeyboardScheme.QUANPIN_NINE_KEY, "a disabled entry falls back to the first Chinese entry");
        check(LanguageKeyCyclePolicy.chineseReturn(null, "quanpin", List.of(KeyboardScheme.JAPANESE), FULL,
            KeyboardScheme.QUANPIN) == KeyboardScheme.QUANPIN, "no Chinese entry enabled uses the fallback");

        check(KeyboardScheme.JAPANESE.otherLanguage() && KeyboardScheme.JAPANESE_NINE_KEY.otherLanguage()
            && KeyboardScheme.KOREAN.otherLanguage() && KeyboardScheme.VIETNAMESE.otherLanguage()
            && KeyboardScheme.TIBETAN.otherLanguage(), "other languages");
        check(!KeyboardScheme.QUANPIN.otherLanguage() && !KeyboardScheme.HANDWRITING.otherLanguage()
            && !KeyboardScheme.CANTONESE.otherLanguage() && !KeyboardScheme.ZHUYIN.otherLanguage()
            && !KeyboardScheme.STROKE.otherLanguage() && !KeyboardScheme.WUBI.otherLanguage(), "Chinese entries");
        System.out.println("Android language key cycle: switch off, no other languages, ring order, faces and return entry passed");
    }
}
