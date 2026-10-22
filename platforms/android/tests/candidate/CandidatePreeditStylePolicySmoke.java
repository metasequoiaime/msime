import app.msime.android.CandidatePreeditStylePolicy;
import app.msime.android.InputSchemeTraits;
import app.msime.android.PhrasePreeditPolicy;

/** 拼写写进输入框后候选栏不再画它、写进输入框的是哪一段，以及候选栏必须留着的两样东西。 */
public final class CandidatePreeditStylePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        String pinyin = CandidatePreeditStylePolicy.PINYIN;
        String empty = CandidatePreeditStylePolicy.EMPTY;

        String editor = CandidatePreeditStylePolicy.EDITOR;
        String candidateBar = CandidatePreeditStylePolicy.CANDIDATE_BAR;
        check(CandidatePreeditStylePolicy.normalize("candidate_bar").equals(candidateBar),
            "the candidate-bar preference is preserved");
        check(CandidatePreeditStylePolicy.normalize("editor").equals(editor)
                && CandidatePreeditStylePolicy.normalize(null).equals(editor)
                && CandidatePreeditStylePolicy.normalize("invalid").equals(editor),
            "missing and invalid preferences use the editor fallback");
        // 桌面候选窗的 `candidate_preedit_style` 默认是 pinyin；这个值不能把触屏拼写挪出输入框。
        check(CandidatePreeditStylePolicy.normalize(pinyin).equals(editor)
                && CandidatePreeditStylePolicy.normalize(empty).equals(editor),
            "candidate_preedit_style values never select the candidate bar");
        check(CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(candidateBar)
                && !CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(editor),
            "the candidate-bar preference keeps spelling out of the editor");

        // 兼容例外只匹配应用完整包名和中文拼音，不影响其他应用、英文与其他语言。
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN}) {
            check(CandidatePreeditStylePolicy.keepsPinyinOnKeyboard("com.shengjingbank.mobile.cust", scheme, false),
                "the incompatible search app keeps pinyin outside its editor");
            for (String app : new String[] {null, "", "app.msime.android.test",
                    "com.shengjingbank.mobile.cust.extra", "com.shengjingbank.enterprisebank"}) {
                check(!CandidatePreeditStylePolicy.keepsPinyinOnKeyboard(app, scheme, false),
                    "a different package keeps inline composition");
            }
            check(!CandidatePreeditStylePolicy.keepsPinyinOnKeyboard("com.shengjingbank.mobile.cust", scheme, true),
                "dedicated English keeps its normal composition");
        }
        for (int scheme : new int[] {InputSchemeTraits.WUBI, InputSchemeTraits.CANTONESE,
                InputSchemeTraits.JAPANESE, InputSchemeTraits.KOREAN, InputSchemeTraits.ZHUYIN, -1}) {
            check(!CandidatePreeditStylePolicy.keepsPinyinOnKeyboard("com.shengjingbank.mobile.cust", scheme, false),
                "the pinyin exception does not change other languages");
        }

        // 拼写类方案的拼写写在输入框里时候选栏不画；没写进去（14 键、编辑器截短）时照常画。
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN,
                InputSchemeTraits.WUBI, InputSchemeTraits.CANTONESE}) {
            check(CandidatePreeditStylePolicy.style(scheme, true).equals(empty),
                "the spelling in the field is not drawn again on the strip, scheme " + scheme);
            check(CandidatePreeditStylePolicy.style(scheme, false).equals(pinyin),
                "a spelling kept out of the field is still drawn, scheme " + scheme);
        }
        check(CandidatePreeditStylePolicy.style(candidateBar, InputSchemeTraits.QUANPIN, true).equals(pinyin),
            "candidate-bar mode keeps pinyin visible even when composition is available");
        check(CandidatePreeditStylePolicy.style(editor, InputSchemeTraits.QUANPIN, true).equals(empty),
            "editor mode hides pinyin after composition is written");
        // 日语读音、就地组字的方案和笔画字形照常画在候选栏上。
        for (int scheme : new int[] {InputSchemeTraits.JAPANESE, InputSchemeTraits.KOREAN,
                InputSchemeTraits.ZHUYIN, InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN,
                InputSchemeTraits.STROKE, -1}) {
            check(CandidatePreeditStylePolicy.style(scheme, true).equals(pinyin),
                "scheme " + scheme + " keeps its composition on the strip");
        }
        check(CandidatePreeditStylePolicy.showsComposition(pinyin)
                && !CandidatePreeditStylePolicy.showsComposition(empty),
            "only empty hides the composition");

        check("nihao".equals(CandidatePreeditStylePolicy.composedText(pinyin, "nihao", false)),
            "a spelling kept out of the field is shown");
        check(CandidatePreeditStylePolicy.composedText(empty, "nihao", false).isEmpty(),
            "a spelling written into the field is not");

        // 本地模式的名字说明正在哪个模式里，去掉它用户就看不出自己在模式里。
        check("表情".equals(CandidatePreeditStylePolicy.composedText(empty, "表情", true)),
            "a local mode's own name stays on the strip");
        check("表情".equals(CandidatePreeditStylePolicy.composedText(pinyin, "表情", true)),
            "and is unaffected when the spelling is drawn");
        // 模式在触发键之后拼写的内容和其他拼写一样。
        check(CandidatePreeditStylePolicy.composedText(empty, "Ksmile", false).isEmpty(),
            "text a local mode is spelling is not drawn once it is in the field");

        check(CandidatePreeditStylePolicy.composedText(empty, null, false).isEmpty()
                && CandidatePreeditStylePolicy.composedText(pinyin, null, true).isEmpty(),
            "a missing title is the empty string rather than a crash");

        // 已选的那一段不是拼写：运行时按这个宿主的请求把它留在文档之外，不画就哪里都看不见。
        String hidden = CandidatePreeditStylePolicy.composedText(empty, "hao", false);
        check("你".equals(PhrasePreeditPolicy.title("你", hidden, false)),
            "the chosen part of the phrase stays on the strip when the spelling is in the field");
        check("你hao".equals(PhrasePreeditPolicy.title("你",
                CandidatePreeditStylePolicy.composedText(pinyin, "hao", false), false)),
            "and is drawn ahead of the spelling when the spelling is drawn");

        // 26 键写引擎的 preedit（双拼按「双拼预编辑」可以是分好词的拼音），没有时退回 editing_text。
        check("ni'hao".equals(CandidatePreeditStylePolicy.inlineSpelling(false, "", "ni'hao", "nihc")),
            "a shuangpin composition writes the engine's preedit, not the keys");
        check("nihao".equals(CandidatePreeditStylePolicy.inlineSpelling(false, "", "", "nihao")),
            "an empty preedit falls back to the keys");
        check("nihao".equals(CandidatePreeditStylePolicy.inlineSpelling(false, "ni'hao", "nihao", "nihao")),
            "outside a key grid the nine-key reading is not used");

        // 九键：输入框里写拼音读音，绝不写按下的数字（产品在设备上看到过 64426 出现在输入框里）。
        String nineKey = CandidatePreeditStylePolicy.inlineSpelling(true, "ni'hao", "64426", "64426");
        check("ni'hao".equals(nineKey), "nine-key writes the engine's pinyin reading: " + nineKey);
        check(!nineKey.matches(".*[0-9].*"), "nine-key composing text has no digits: " + nineKey);
        String withPrefix = PhrasePreeditPolicy.composing("你", CandidatePreeditStylePolicy.inlineSpelling(
            true, "hao", "426", "426"));
        check("你hao".equals(withPrefix) && !withPrefix.matches(".*[0-9].*"),
            "the chosen part leads the nine-key reading, still without digits: " + withPrefix);
        String shuangpinGrid = CandidatePreeditStylePolicy.inlineSpelling(true, "zhong'g", "8146", "8146");
        check("zhong'g".equals(shuangpinGrid), "nine-key shuangpin writes its reading too: " + shuangpinGrid);
        check("ni'426".equals(CandidatePreeditStylePolicy.inlineSpelling(true, "", "ni'426", "64426")),
            "only without a reading does nine-key fall back to the engine's preedit");
        System.out.println("Android candidate preedit style policy passed");
    }
}
