import app.msime.android.ChineseSymbolFaces;
import app.msime.android.InputSchemeTraits;
import app.msime.android.KeyFace;
import app.msime.android.KeyFaceLayout;
import app.msime.android.KeyFaceLive;
import app.msime.android.KeyFaceSlot;
import app.msime.android.KeyFaceSpec;
import app.msime.android.KeyFaceSpecs;
import app.msime.android.KeyboardIconKey;
import app.msime.android.KeyboardScheme;
import app.msime.android.NineKeyCompositionActionPolicy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 键面规格（{@link KeyFaceSpecs}）双向核对：每种布局、每种 {@link KeyFaceLive} 下，此刻的状态都在 {@code states} 里（列少了，键画上时会被键组当作漏网的字），{@code states} 里的每个状态也都有一种情形会走到（列多了，键会按画不出来的字平白缩小，例如 26 键的 ⇧ 不该列 14/18 键的「符」）。另外钉住几条出过错的结论。
 */
public final class KeyFaceSpecsSmoke {
    private KeyFaceSpecsSmoke() { }

    private static final List<KeyFaceSpec<?>> SPECS = List.of(KeyFaceSpecs.SHIFT, KeyFaceSpecs.FOURTEEN_SEPARATOR,
        KeyFaceSpecs.NINE_ONE, KeyFaceSpecs.JAPANESE_LAYER, KeyFaceSpecs.JAPANESE_VARIANTS, KeyFaceSpecs.JAPANESE_SPACE,
        KeyFaceSpecs.JAPANESE_RETURN, KeyFaceSpecs.LANGUAGE, KeyFaceSpecs.SYMBOL_PANEL, KeyFaceSpecs.LAYER, KeyFaceSpecs.RETURN);

    public static void main(String[] args) {
        List<KeyFaceLayout> layouts = layouts();
        for (KeyFaceSpec<?> spec : SPECS) twoWay(spec, layouts);

        // 26 键：⇧ 只在全拼可分词时写「分词」；功能键组里没有 14/18 键的「符」、日语九键的字。
        KeyFaceLayout standard = layout(0, false, NineKeyCompositionActionPolicy.SEPARATE, false, List.of());
        check(KeyFaceSpecs.functionTexts(standard, KeyFaceSlot.MAIN).equals(List.of(KeyFaceSpecs.SEPARATOR_FACE)),
            "26 键的功能键组只应有 ⇧ 换成的「分词」：" + KeyFaceSpecs.functionTexts(standard, KeyFaceSlot.MAIN));
        check(KeyFaceSpecs.functionTexts(standard, KeyFaceSlot.TOP_RIGHT).isEmpty(), "26 键的功能键组不应为「辅」留位置");
        KeyFaceLayout shiftKept = layout(0, true, NineKeyCompositionActionPolicy.SEPARATE, false, List.of());
        check(KeyFaceSpecs.functionTexts(shiftKept, KeyFaceSlot.MAIN).isEmpty(), "全拼 Shift 切大小写时 ⇧ 不写字");
        check(KeyFaceSpecs.SHIFT.initial(standard).text() == null && KeyFaceSpecs.SHIFT.initial(standard).icon()
            == KeyboardIconKey.Kind.SHIFT, "⇧ 建键时画图标");
        // 14 键：符、分词和「辅」。
        KeyFaceLayout fourteen = layout(8, false, NineKeyCompositionActionPolicy.SEPARATE, false, List.of());
        List<String> grid = KeyFaceSpecs.functionTexts(fourteen, KeyFaceSlot.MAIN);
        check(grid.contains("符") && grid.contains("分词"), "14 键的功能键组缺符或分词：" + grid);
        check(KeyFaceSpecs.functionTexts(fourteen, KeyFaceSlot.TOP_RIGHT).equals(List.of("辅")), "14 键的分词键角标应只有「辅」");
        // 日语九键：两侧列和左下角会变的字都在，九键的「@#」不在。
        KeyFaceLayout japanese = layout(2, false, NineKeyCompositionActionPolicy.SEPARATE, false, List.of());
        List<String> side = KeyFaceSpecs.functionTexts(japanese, KeyFaceSlot.MAIN);
        for (String face : new String[] {"123", "あいう", "小゛゜", "（）", "空白", "変換", "中", "英"})
            check(side.contains(face), "日语九键的功能键组缺 " + face);
        // 九键 1 键：只列设置选的那个动作。
        KeyFaceLayout rewrite = layout(1, false, NineKeyCompositionActionPolicy.REWRITE, false, List.of());
        check(KeyFaceSpecs.NINE_ONE.texts(rewrite, KeyFaceSlot.MAIN).equals(List.of("@#", "重输", "1")),
            "选重输时 1 键应是 @#、重输、1：" + KeyFaceSpecs.NINE_ONE.texts(rewrite, KeyFaceSlot.MAIN));
        // 语言键：关着轮换只有中、英；开着时加上可选的其他语言。
        KeyFaceLayout cycling = layout(2, false, NineKeyCompositionActionPolicy.SEPARATE, true,
            List.of(KeyboardScheme.QUANPIN, KeyboardScheme.JAPANESE_NINE_KEY, KeyboardScheme.KOREAN));
        check(KeyFaceSpecs.LANGUAGE.texts(japanese, KeyFaceSlot.MAIN).equals(List.of("中", "英")), "不轮换时语言键只有中、英");
        check(KeyFaceSpecs.LANGUAGE.texts(cycling, KeyFaceSlot.MAIN).equals(List.of("中", "英", "あ", "한")),
            "轮换时语言键应加上可选的其他语言：" + KeyFaceSpecs.LANGUAGE.texts(cycling, KeyFaceSlot.MAIN));
        // 底行：符键只在九键双拼上列「分词」（26 键上与中/英一样只为「符」留位置）；123 键只列这副键盘的层标题。
        check(KeyFaceSpecs.SYMBOL_PANEL.texts(standard, KeyFaceSlot.MAIN).equals(List.of("符")), "26 键的符键只应有「符」");
        KeyFaceLayout shuangpinNine = new KeyFaceLayout(1, false, NineKeyCompositionActionPolicy.SEPARATE, true,
            ChineseSymbolFaces.Marks.DEFAULT, false, List.of(), List.of(), true);
        check(KeyFaceSpecs.SYMBOL_PANEL.texts(shuangpinNine, KeyFaceSlot.MAIN).equals(List.of("符", "分词")),
            "九键双拼的符键应有「符」「分词」");
        check(!KeyFaceSpecs.LAYER.texts(standard, KeyFaceSlot.MAIN).contains("あいう"), "26 键的 123 键不应有「あいう」");
        check(KeyFaceSpecs.LAYER.texts(standard, KeyFaceSlot.MAIN).contains("123")
            && KeyFaceSpecs.LAYER.texts(standard, KeyFaceSlot.MAIN).contains("返回"), "26 键的 123 键缺 123 或返回");
        check(KeyFaceSpecs.LAYER.texts(japanese, KeyFaceSlot.MAIN).contains("あいう"), "日语九键的 123 键缺「あいう」");
        // 回车：组字时的两种字、两种角标；画图标时不占文字。
        check(KeyFaceSpecs.RETURN.texts(standard, KeyFaceSlot.MAIN).equals(List.of("确认", "確定")), "回车的字应是确认、確定");
        check(KeyFaceSpecs.RETURN.texts(standard, KeyFaceSlot.TOP_RIGHT).equals(List.of("换行", "译")), "回车的角标应是换行、译");
        // 底行的语言键不进 26 键的功能键组。
        check(!KeyFaceSpecs.functionTexts(standard, KeyFaceSlot.MAIN).contains("中"), "26 键的功能键组不应有中/英");
        // 画图标的状态不进内容域。
        check(KeyFace.icon(KeyboardIconKey.Kind.SHIFT).mainText().isEmpty(), "图标不占文字");
        System.out.println("KeyFaceSpecsSmoke passed");
    }

    /** 每种布局：此刻的状态都在列表里，列表里的每个状态都走得到。 */
    private static <S> void twoWay(KeyFaceSpec<S> spec, List<KeyFaceLayout> layouts) {
        for (KeyFaceLayout layout : layouts) {
            if (!spec.on(layout)) continue;
            List<S> states = spec.states(layout);
            check(!states.isEmpty(), "规格在布局 " + layout.touchLayout() + " 上没有状态");
            Set<S> reached = new HashSet<>();
            for (KeyFaceLive live : lives(layout)) {
                S state = spec.state(layout, live);
                check(states.contains(state), "状态 " + state + " 不在布局 " + layout.touchLayout() + " 的列表里：" + live);
                reached.add(state);
            }
            for (S state : states)
                check(reached.contains(state), "布局 " + layout.touchLayout() + " 列了走不到的状态 " + state + "（"
                    + spec.face(state) + "）");
        }
    }

    /** 每种触屏布局一个基准，再各改一项设置（全拼 Shift、九键 1 键的动作、中英键轮换和全部方案、九键双拼）。 */
    private static List<KeyFaceLayout> layouts() {
        List<KeyFaceLayout> layouts = new ArrayList<>();
        List<KeyboardScheme> chinese = List.of(KeyboardScheme.QUANPIN, KeyboardScheme.XIAOHE);
        List<KeyboardScheme> all = List.of(KeyboardScheme.values());
        String separate = NineKeyCompositionActionPolicy.SEPARATE;
        for (int touchLayout = 0; touchLayout <= 9; touchLayout++) {
            layouts.add(layout(touchLayout, false, separate, false, chinese, false));
            layouts.add(layout(touchLayout, true, separate, false, chinese, false));
            layouts.add(layout(touchLayout, false, NineKeyCompositionActionPolicy.REWRITE, false, chinese, false));
            layouts.add(layout(touchLayout, false, separate, false, all, false));
            layouts.add(layout(touchLayout, false, separate, true, all, false));
            layouts.add(layout(touchLayout, false, separate, false, chinese, true));
        }
        return layouts;
    }

    private static final Map<List<KeyboardScheme>, List<KeyFaceLive>> LIVES = new HashMap<>();

    /** 这副键盘上可能的每种此刻状态：选中的方案只取这副键盘可选的（或没有）。 */
    private static List<KeyFaceLive> lives(KeyFaceLayout layout) {
        return LIVES.computeIfAbsent(layout.schemes(), KeyFaceSpecsSmoke::lives);
    }

    /** 选中的方案取这副键盘可选的里有代表性的几个（没有、全拼、双拼）和每种其他语言（语言键轮换时画它们的字）。 */
    private static List<KeyFaceLive> lives(List<KeyboardScheme> schemes) {
        List<KeyboardScheme> selected = new ArrayList<>();
        selected.add(null);
        for (KeyboardScheme scheme : schemes)
            if (scheme == KeyboardScheme.QUANPIN || scheme == KeyboardScheme.XIAOHE || scheme.otherLanguage()) selected.add(scheme);
        List<KeyFaceLive> lives = new ArrayList<>();
        // 14 个开关：组字、符号层、英文、中文会话、无临时模式、锁定大写、辅助码、数字九宫格、注音共用行、中文键面、日语、回车确认、长按译文、长按换行。
        // 全部开关的组合各走一遍（不选方案）；选中的方案和引擎方案只和前五个开关（组字、符号层、英文、中文会话、无临时模式）组合，它们只和这几个开关一起起作用。
        for (int bits = 0; bits < 1 << 14; bits++) lives.add(live(bits, -1, null));
        for (int bits = 0; bits < 1 << 5; bits++)
            for (int scheme : new int[] {-1, InputSchemeTraits.WUBI})
                for (KeyboardScheme chosen : selected) lives.add(live(bits, scheme, chosen));
        return lives;
    }

    private static KeyFaceLayout layout(int touchLayout, boolean shift, String action, boolean cycles,
            List<KeyboardScheme> schemes) {
        return layout(touchLayout, shift, action, cycles, schemes, false);
    }

    private static KeyFaceLayout layout(int touchLayout, boolean shift, String action, boolean cycles,
            List<KeyboardScheme> schemes, boolean shuangpin) {
        return new KeyFaceLayout(touchLayout, shift, action, true, ChineseSymbolFaces.Marks.DEFAULT, cycles, schemes,
            List.of(), shuangpin);
    }

    private static KeyFaceLive live(int bits, int scheme, KeyboardScheme chosen) {
        return new KeyFaceLive(bit(bits, 0), bit(bits, 1), bit(bits, 2), bit(bits, 3), bit(bits, 4), bit(bits, 5), bit(bits, 6),
            scheme, chosen, bit(bits, 7), bit(bits, 8), bit(bits, 9), bit(bits, 10), bit(bits, 11), bit(bits, 12), bit(bits, 13));
    }

    private static boolean bit(int bits, int index) {
        return (bits & (1 << index)) != 0;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
