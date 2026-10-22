package app.msime.android;

/**
 * 候选栏上画不画正在拼写的内容，以及写进输入框的是哪一段拼写。
 *
 * <p>拼写可以作为组字写进输入框，也可以只画在候选栏上，由共享偏好 `touch_spelling_position` 决定，缺省写进输入框。不读 `candidate_preedit_style`：那是桌面候选窗画不画拼音行，默认值是画，拿它当位置开关会让所有没动过设置的用户默认把拼写挪出输入框。盛京银行的全拼、双拼因搜索框不区分组字而留在键盘上，见 {@link #keepsPinyinOnKeyboard}。只有拼写类方案（全拼、双拼、五笔、粤拼，与 iOS `hasSpellingCaret` 同一组）这样做：日语读音、韩语音节这类就地组字的方案和笔画字形照常画在候选栏上；全拼 14 键的组字是组码字母（`bugao`），不写进输入框，读音仍画在候选栏上；编辑器截短了组字、整段组字只能留在键盘上时也照常画。
 *
 * <p>有两样东西不归这里管。
 *
 * <p>正在拼的词里已经选中的那一段留在候选栏上。运行时按这个宿主的请求（`phrase_preedit`）把它留在组字里，正是为了让用户接着拼后半截；不画它的话，用户已经选中的字既不在文档里也不在屏幕上，`scripts/test-phrase-preedit-hosts.py` 防的就是这种状态。
 *
 * <p>本地输入模式的名字（表情、颜文字、日期时间等）也留着。它说明正在哪个模式里，不是正在拼写的内容，去掉它用户就看不出自己在一个模式里。模式在触发键之后拼写的内容和其他拼写一样不画。
 */
public final class CandidatePreeditStylePolicy {
    public static final String PINYIN = "pinyin";
    public static final String EMPTY = "empty";
    /** 共享偏好 `touch_spelling_position` 的两个值。 */
    public static final String EDITOR = "editor";
    public static final String CANDIDATE_BAR = "candidate_bar";

    private CandidatePreeditStylePolicy() {}

    /** 不区分组字的应用按精确包名豁免；专用英文与其他语言仍按各自规则写入。 */
    public static boolean keepsPinyinOnKeyboard(String packageName, int scheme, boolean dedicatedEnglish) {
        return !dedicatedEnglish && (scheme == InputSchemeTraits.QUANPIN
            || scheme == InputSchemeTraits.SHUANGPIN)
            && "com.shengjingbank.mobile.cust".equals(packageName);
    }

    /** 拼写写进输入框、候选栏不再画它的方案：全拼（含九键、九键双拼）、双拼、五笔和粤拼。 */
    public static boolean writesSpelling(int scheme) {
        return scheme == InputSchemeTraits.QUANPIN || scheme == InputSchemeTraits.SHUANGPIN
            || scheme == InputSchemeTraits.WUBI || scheme == InputSchemeTraits.CANTONESE;
    }

    /** 共享偏好只接受两个值；缺失和未知值按输入框回退，保持 Android 原来的行为。 */
    public static String normalize(String position) {
        return CANDIDATE_BAR.equals(position) ? CANDIDATE_BAR : EDITOR;
    }

    /** 拼写是否只显示在候选栏，不写入编辑器。 */
    public static boolean keepsSpellingOnKeyboard(String position) {
        return CANDIDATE_BAR.equals(normalize(position));
    }

    /**
     * 候选栏的样式：拼写类方案的拼写已经写在输入框里时为 {@link #EMPTY}，其余为 {@link #PINYIN}。
     *
     * @param spellingInEditor 这一刻的组字确实写在了输入框里（不是 14 键，编辑器也没有把它截掉）
     */
    public static String style(int scheme, boolean spellingInEditor) {
        return writesSpelling(scheme) && spellingInEditor ? EMPTY : PINYIN;
    }

    /** 按拼写位置偏好和实际组字状态决定候选栏样式。 */
    public static String style(String position, int scheme, boolean spellingInEditor) {
        return keepsSpellingOnKeyboard(position) ? PINYIN : style(scheme, spellingInEditor);
    }

    /** 候选栏画不画组字。 */
    public static boolean showsComposition(String style) {
        return !EMPTY.equals(style);
    }

    /**
     * 画在候选栏上的组字：正在拼写的内容，或者什么也不画。
     *
     * @param modeName 为真时这段文字是本地模式自己的名字，而不是拼写
     */
    public static String composedText(String style, String text, boolean modeName) {
        if (text == null) return "";
        if (modeName || showsComposition(style)) return text;
        return "";
    }

    /**
     * 写进输入框的拼写（不含已选的那一段）。九键和九键双拼（{@code keyGrid}）的 `editing_text` 和 `preedit` 是按下的数字（64426），写的是引擎给的拼音读音 `nine_key_reading`（ni'hao），与候选栏原来画的读音是同一段；只有引擎没给读音时才退回 `preedit`。其余写 `preedit`（双拼按「双拼预编辑」可以是 ni'hao），它为空时退回 `editing_text`。
     */
    public static String inlineSpelling(boolean keyGrid, String nineKeyReading, String preedit, String editingText) {
        if (keyGrid && nineKeyReading != null && !nineKeyReading.isEmpty()) return nineKeyReading;
        if (preedit != null && !preedit.isEmpty()) return preedit;
        return editingText == null ? "" : editingText;
    }
}
