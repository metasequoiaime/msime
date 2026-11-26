package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import app.msime.android.core.InputViewValuePolicy;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}），一个角色一组（{@link KeyFaceRole}：模板、提示位置、上限都写在角色表里）：建键行时清空，键放进行里时按角色放进对应的组（{@link #attach}）。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains}、{@link NineKeyFaceDomains}（含笔画键盘、注音九键和日语九键）、{@link KeyFaceSpecs} 和 {@link SideColumnKeyFaceDomains}，都按这副键盘的 {@link KeyFaceLayout} 列出。键面会变的键（⇧、14/18 键的分词键、九键的 1 键、日语九键的几个侧列键）由 {@link #show} 按它们的 {@link KeyFaceSpec} 设上文字或图标。
 *
 * <p>功能键组的字不大过同一副键盘的字母组（{@link KeyFaceRole#leaders}、{@link KeyFaceGroup#capMainAt}）。⇧、⌫ 和底行的 0 是常驻的键，建键行时会挪到别的地方（123 层、底行），所以清空时先把它们拿出组，放回原处时再放进新组（{@link KeyFaceRole#movable}）。
 *
 * <p>皮肤字体、键帽形状、字体缩放或 {@link KeyFaceLayout} 里的设置（角标开关、引号样式、双拼提示表……）变了，{@link #refresh} 在下一次渲染时换掉组的环境和内容域，组里的键随之重画，不重建键行。键画上列表里没有的字是列表漏了（{@link KeyFaceGroup#include}），记一行日志。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> NO_HINTS = Set.of();
    private static final Set<KeyFaceSlot> CORNER = Set.of(KeyFaceSlot.TOP_RIGHT);
    /** #+= 层和 ー 的组一开始的内容域：空的，放进来的每个键把自己的文字补进去（{@link KeyFaceGroup#attach}），重建时整组换掉。 */
    private static final KeyFaceDomain ATTACHED_TEXTS = new KeyFaceDomain(Map.of());

    /** 决定内容域、字体和环境的几项；都没变时 {@link #refresh} 什么也不做。 */
    private record Inputs(float density, float fontScale, boolean monospaced, KeyOutline outline, KeyFaceLayout layout) { }

    /** 一个角色这副键盘上的组：第一次放键进来时建，记着建组时的输入。 */
    private static final class Built {
        final KeyFaceGroup group;
        Inputs inputs;

        Built(KeyFaceGroup group, Inputs inputs) {
            this.group = group;
            this.inputs = inputs;
        }
    }

    private final MSIMEInputService s;
    private final EnumMap<KeyFaceRole, Built> groups = new EnumMap<>(KeyFaceRole.class);
    /** 这副键盘是日语九键：功能键放进 {@link KeyFaceRole#JAPANESE_FUNCTIONS}。 */
    private boolean japaneseFunctions;
    /** 符号栏这一次建出来的表（{@link #beginSideSymbols}）：符号栏两个角色的内容域。 */
    private KeyFaceDomain sideSymbolsDomain = ATTACHED_TEXTS;
    private KeyFaceDomain sideCornerDomain = ATTACHED_TEXTS;
    /** 放进过组的键里建键行后还会留着用的那些（⇧、⌫、底行的 0 等），清空时拿出组。 */
    private final List<KeyboardPressButton> movableKeys = new java.util.ArrayList<>();
    /** 已经记过日志的漏网文字（键组模板名、位置、文字），每种只记一次。 */
    private final Set<String> strays = new java.util.HashSet<>();

    ImeKeyFaces(MSIMEInputService s) {
        this.s = s;
        KeyFaceGroup.setStrayListener((group, slot, text) -> {
            if (strays.add(group.template().name() + " " + slot + " " + text))
                ImeLog.w("key face list misses " + group.template().name() + " " + slot + " 「" + text + "」");
        });
    }

    /** 建键行之前调用：上一副键盘的组不再用，新键放进新组。底行的组（{@link KeyFaceRole#persistent}）留着，它们的键建键行之后还在用，只是上限要等新键盘的字母组建好再连上。 */
    void reset() {
        for (KeyboardPressButton key : movableKeys) if (key.faceGroup() != null) key.setFaceGroup(null, NO_HINTS);
        movableKeys.clear();
        groups.keySet().removeIf(role -> !role.persistent());
        japaneseFunctions = false;
        sideSymbolsDomain = ATTACHED_TEXTS;
        sideCornerDomain = ATTACHED_TEXTS;
        link();
    }

    /** 换了一套控件（{@link ImeBottomRow#ensureFaces} 新建了空格、回车）：底行的组作废，下次排布时放进新组。 */
    void resetBottomRow() {
        groups.keySet().removeIf(KeyFaceRole::persistent);
    }

    /**
     * 底行的一个常驻键放进它自己的组（每个键一个角色）；已经在里面时只按此刻的状态重设键面。每次排布都经过这里：{@link ImeBottomRow#placeKey}、右侧大回车外框和注音九键两列挪键、#+= 层把空格和回车挪进自己的最后一行。地球键自己画（按键高缩放的描边地球），不放进组；底行的 0 由九键数字组放（{@link KeyFaceRole#NINE_DIGITS}）。
     */
    void attachBottomRow(android.widget.Button key, KeyboardActionRow.DesignSlot slot) {
        switch (slot) {
            case GLOBE, ZERO -> { }
            case COMMA -> attach(key, KeyFaceRole.BOTTOM_COMMA);
            case PERIOD -> attach(key, KeyFaceRole.BOTTOM_PERIOD);
            case SPACE -> attach(key, KeyFaceRole.BOTTOM_SPACE);
            case RETURN -> attach(key, KeyFaceRole.RETURN, KeyFaceRole.RETURN.slots(), KeyFaceSpecs.RETURN);
            case SYMBOL_PANEL -> attach(key, KeyFaceRole.BOTTOM_SYMBOL_PANEL, NO_HINTS, KeyFaceSpecs.SYMBOL_PANEL);
            case LANGUAGE -> attach(key, KeyFaceRole.BOTTOM_LANGUAGE, NO_HINTS, KeyFaceSpecs.LANGUAGE);
            case REWRITE -> attach(key, KeyFaceRole.BOTTOM_REWRITE);
            case EMOJI -> attach(key, KeyFaceRole.BOTTOM_EMOJI);
            case DELETE -> attach(key, KeyFaceRole.BOTTOM_DELETE);
            default -> attach(key, KeyFaceRole.BOTTOM_LAYER, NO_HINTS, KeyFaceSpecs.LAYER);
        }
    }

    /** 右侧大回车外框里的回车（同一个回车键，挪回功能行时 {@link #attachBottomRow} 再放回普通回车组）。 */
    void attachTallReturn(android.widget.Button key) {
        attach(key, KeyFaceRole.TALL_RETURN, KeyFaceRole.TALL_RETURN.slots(), KeyFaceSpecs.RETURN);
    }

    /** 这副键盘上已经建了的组，按角色（设备检查读它，核对键格和漏网的字）。 */
    Map<KeyFaceRole, KeyFaceGroup> groups() {
        Map<KeyFaceRole, KeyFaceGroup> built = new EnumMap<>(KeyFaceRole.class);
        for (Map.Entry<KeyFaceRole, Built> entry : groups.entrySet()) built.put(entry.getKey(), entry.getValue().group);
        return built;
    }

    /** 文字固定的键按 {@code role} 放进这副键盘的组；已经在里面时什么也不做（底行的 0 每次渲染都会经过这里）。不是 {@link KeyboardPressButton} 的键照旧自己画。 */
    void attach(android.widget.Button key, KeyFaceRole role) {
        attach(key, role, role.slots(), null);
    }

    private void attach(android.widget.Button key, KeyFaceRole role, Set<KeyFaceSlot> slots, KeyFaceSpec<?> spec) {
        if (!(key instanceof KeyboardPressButton press)) return;
        KeyFaceGroup group = group(role);
        if (press.faceGroup() != group) press.setFaceGroup(group, slots, spec == null);
        if (spec != null) show(key, spec);
        if (role.movable() && !movableKeys.contains(press)) movableKeys.add(press);
        link();
    }

    /** 键区里的一个功能键（⇧、⌫、分词键、九键右列的重输），文字固定。 */
    void attachFunction(android.widget.Button key) {
        attach(key, functionRole(), NO_HINTS, null);
    }

    /** 键面会变的功能键：按 {@code spec} 放进组（它的全部状态已经在组的内容域里），随即按此刻的状态设上键面，常驻的键（⇧）不会带着上一副键盘的字进来。 */
    void attachFunction(android.widget.Button key, KeyFaceSpec<?> spec) {
        attach(key, functionRole(), spec.drawsCorner() ? CORNER : NO_HINTS, spec);
    }

    private KeyFaceRole functionRole() {
        return japaneseFunctions ? KeyFaceRole.JAPANESE_FUNCTIONS : KeyFaceRole.FUNCTIONS;
    }

    /**
     * 按 {@code spec} 和此刻的状态设上键面：画文字的键换上这段字（节点文字随之换），画图标的键（{@link KeyboardIconKey}）换图标；{@link KeyFaceSpec#drawsCorner} 时连右上角一起设。渲染时调用，代替各处自己写字。
     */
    void show(android.widget.Button key, KeyFaceSpec<?> spec) {
        if (key == null) return;
        KeyFace face = spec.face(layout(), live());
        if (key instanceof KeyboardIconKey icon) {
            icon.setFace(face, spec.ownsIconKind());
            return;
        }
        if (face.text() != null && !face.text().contentEquals(key.getText())) key.setText(face.text());
        if (spec.drawsCorner() && key instanceof KeyHintButton hint) hint.setCornerHint(face.corner());
    }

    /** 建键时的文字：{@code spec} 此刻的状态写的字（画图标时为空串）。 */
    String text(KeyFaceSpec<?> spec) {
        return spec.face(layout(), live()).mainText();
    }

    /** 建日语九键之前调用（在放进第一个功能键之前）：功能键放进 {@link KeyFaceRole#JAPANESE_FUNCTIONS}。 */
    void useJapaneseFunctions() {
        japaneseFunctions = true;
    }

    /**
     * 九键、笔画或数字九宫格左侧符号栏这一次建出来的符号：{@code symbols} 是整栏的表，{@code zeroSymbol} 是右上角印 0 的那一格（大回车外框接在末尾的 ！），没有时为 {@code null}。每次建栏先调这里，再把每一格按 {@link KeyFaceRole#SIDE_SYMBOLS} 或 {@link KeyFaceRole#SIDE_CORNER} 放进来；一副键盘只有一条符号栏。
     *
     * <p>带 0 的那一格自己一组，主标签以整栏为上限，所以和整栏一样大、不会更大：整栏按最宽的符号（……）的框判断碰不碰右上角，放在一组里时这个框会碰上只有 ！ 才有的 0，整栏都要为它左移或缩小。
     */
    void beginSideSymbols(List<String> symbols, String zeroSymbol) {
        groups.remove(KeyFaceRole.SIDE_SYMBOLS);
        groups.remove(KeyFaceRole.SIDE_CORNER);
        sideSymbolsDomain = SideColumnKeyFaceDomains.symbols(symbols, false);
        sideCornerDomain = zeroSymbol == null ? ATTACHED_TEXTS : SideColumnKeyFaceDomains.symbols(List.of(zeroSymbol), true);
    }

    /** {@code role} 这副键盘上的组，还没有时按此刻的输入建。 */
    private KeyFaceGroup group(KeyFaceRole role) {
        Built built = groups.get(role);
        if (built == null) {
            Inputs inputs = inputs();
            built = new Built(new KeyFaceGroup(role.template(), environment(inputs), domain(role, inputs), measurer(role, inputs)),
                inputs);
            groups.put(role, built);
        }
        return built.group;
    }

    /** 每个组的主标签以角色表里的领头组为上限（这副键盘上有的第一个），图标同样（{@link KeyFaceGroup#capMainAt}）。 */
    private void link() {
        for (Map.Entry<KeyFaceRole, Built> entry : groups.entrySet()) {
            KeyFaceGroup leader = null;
            for (KeyFaceRole role : entry.getKey().leaders()) {
                Built candidate = groups.get(role);
                if (candidate != null) {
                    leader = candidate.group;
                    break;
                }
            }
            if (!entry.getKey().leaders().isEmpty()) entry.getValue().group.capMainAt(leader);
        }
    }

    /** {@code role} 在这副键盘上的内容域。 */
    private KeyFaceDomain domain(KeyFaceRole role, Inputs inputs) {
        KeyFaceLayout layout = inputs.layout();
        return switch (role) {
            case LETTERS -> LetterKeyFaceDomains.letters(layout);
            case FOURTEEN_LETTERS -> LetterKeyFaceDomains.gridLetters(false, layout);
            case EIGHTEEN_LETTERS -> LetterKeyFaceDomains.gridLetters(true, layout);
            case NUMBER_ROW -> LetterKeyFaceDomains.numberRow();
            case FUNCTIONS, JAPANESE_FUNCTIONS -> KeyFaceSpecs.functionDomain(layout);
            case NINE_LETTERS -> NineKeyFaceDomains.letters(layout);
            case NINE_DIGITS -> NineKeyFaceDomains.digits();
            case NINE_SHUANGPIN -> NineKeyFaceDomains.shuangpin(s.nineKeyShuangpinFaces());
            case KOREAN -> LetterKeyFaceDomains.korean();
            case DACHEN -> LetterKeyFaceDomains.dachen();
            case STROKES -> NineKeyFaceDomains.strokes();
            case STROKE_KEYS -> NineKeyFaceDomains.strokeKeys();
            case ZHUYIN_TONES -> NineKeyFaceDomains.zhuyinTones();
            case ZHUYIN_SOUNDS -> NineKeyFaceDomains.zhuyinSounds();
            case ZHUYIN_MARKS -> NineKeyFaceDomains.zhuyinMarks();
            case JAPANESE_KANA -> NineKeyFaceDomains.japaneseKana(false);
            case JAPANESE_DIGITS -> NineKeyFaceDomains.japaneseKana(true);
            case SIDE_SYMBOLS -> sideSymbolsDomain;
            case SIDE_CORNER -> sideCornerDomain;
            case LONG_VOWEL, LAYER_TOP, LAYER_CHARACTERS, BOTTOM_REWRITE, BOTTOM_EMOJI, BOTTOM_DELETE -> ATTACHED_TEXTS;
            case BOTTOM_SYMBOL_PANEL -> KeyFaceSpecs.domain(KeyFaceSpecs.SYMBOL_PANEL, layout);
            case BOTTOM_LAYER -> KeyFaceSpecs.domain(KeyFaceSpecs.LAYER, layout);
            case BOTTOM_LANGUAGE -> KeyFaceSpecs.domain(KeyFaceSpecs.LANGUAGE, layout);
            case RETURN, TALL_RETURN -> KeyFaceSpecs.domain(KeyFaceSpecs.RETURN, layout);
            case BOTTOM_COMMA -> BottomRowKeyFaceDomains.comma();
            case BOTTOM_PERIOD -> BottomRowKeyFaceDomains.period();
            case BOTTOM_SPACE -> BottomRowKeyFaceDomains.space();
        };
    }

    /** 每次渲染时调用：设置、皮肤字体或字体缩放变了就换掉组的内容域；没变时只比较几项。 */
    void refresh() {
        // 每次渲染都会走到这里（一次渲染几回）：KeyFaceLayout 只读一次，各组拿它比较。
        Inputs now = inputs();
        for (Map.Entry<KeyFaceRole, Built> entry : groups.entrySet()) {
            Built built = entry.getValue();
            if (now.equals(built.inputs)) continue;
            built.inputs = now;
            built.group.replace(environment(now), measurer(entry.getKey(), now), domain(entry.getKey(), now));
        }
    }

    private Inputs inputs() {
        Configuration configuration = s.getResources().getConfiguration();
        float density = s.getResources().getDisplayMetrics().density;
        return new Inputs(density, KeyboardTypographyPolicy.keyboardFontScale(configuration.fontScale),
            s.skin != null && s.skin.monospaced(), KeyOutline.forSkin(s.skin, density), layout());
    }

    /** 这副键盘的 {@link KeyFaceLayout}：显示的触屏布局和决定键面能画哪些字的设置。 */
    KeyFaceLayout layout() {
        List<String> hints = new java.util.ArrayList<>();
        if (s.shuangpinKeyHintsEnabled && s.shuangpinHints != null)
            for (String hint : s.shuangpinHints.values()) if (hint != null && !hint.isEmpty() && !hints.contains(hint)) hints.add(hint);
        return new KeyFaceLayout(s.displayedTouchLayout(s.view), s.quanpinShiftEnabled, s.nineKeyCompositionAction,
            s.localSettings.bool(AndroidLocalSettings.LETTER_CORNER_HINTS), s.punctuationMarks(),
            s.localSettings.bool(AndroidLocalSettings.LANGUAGE_KEY_CYCLE), s.visibleSchemes, hints, s.nineKeyShuangpinDrawn());
    }

    /** 此刻随打字、切层变化的几项（{@link KeyFaceLive}）。 */
    KeyFaceLive live() {
        String localMode = s.view == null ? "none" : InputViewValuePolicy.textOr(s.view, "local_mode", "none");
        return new KeyFaceLive(s.hasEngineComposition(), s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS, s.dedicatedEnglish,
            s.session != 0 && s.view != null && !s.dedicatedEnglish && !EditorPolicy.password(s.editorInputType),
            "none".equals(localMode), s.letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK,
            ImeGridHelpcodeEntry.available(s), s.view == null ? -1 : InputViewValuePolicy.scheme(s.view, -1), s.selectedScheme,
            s.imeLetterRows.numberPadDrawn(), s.imeLetterRows.sharedSymbolRowsDrawn(),
            KeyboardLayout.chineseFace(s.view == null ? -1 : InputViewValuePolicy.scheme(s.view, -1), s.dedicatedEnglish),
            s.japaneseSchemeActive(), s.returnKeyConfirms(), s.returnHoldCommitsTranslation(), s.returnHoldInsertsNewline());
    }

    private static KeyFaceEnvironment environment(Inputs inputs) {
        return new KeyFaceEnvironment(inputs.density(), inputs.fontScale(), inputs.outline());
    }

    /** 九键的键用 `sans-serif-medium`（{@link KeyFaceRole#nineKeyTypeface}），等宽皮肤用等宽字体，与样式通道给键帽的字体相同（ImeStyler）。 */
    private static KeyFaceMeasurer measurer(KeyFaceRole role, Inputs inputs) {
        if (role.bold())
            return new PaintKeyFaceMeasurer(Typeface.create(inputs.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT, Typeface.BOLD));
        if (inputs.monospaced()) return new PaintKeyFaceMeasurer(Typeface.MONOSPACE);
        return new PaintKeyFaceMeasurer(role.nineKeyTypeface() ? Typeface.create("sans-serif-medium", Typeface.NORMAL)
            : Typeface.DEFAULT);
    }
}
