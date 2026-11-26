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
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * 键面规格（{@link KeyFaceSpecs}）双向核对：每种布局、每种 {@link KeyFaceLive} 下，此刻的状态都在 {@code states} 里（列少了，键画上时会被键组当作漏网的字），{@code states} 里的每个状态也都有一种情形会走到（列多了，键会按画不出来的字平白缩小，例如 26 键的 ⇧ 不该列 14/18 键的「符」）。另外钉住几条出过错的结论。
 */
public final class KeyFaceSpecsSmoke {
    private KeyFaceSpecsSmoke() { }

    private static final List<KeyFaceSpec<?>> SPECS = List.of(KeyFaceSpecs.SHIFT, KeyFaceSpecs.FOURTEEN_SEPARATOR,
        KeyFaceSpecs.NINE_ONE, KeyFaceSpecs.JAPANESE_LAYER, KeyFaceSpecs.JAPANESE_VARIANTS, KeyFaceSpecs.JAPANESE_SPACE,
        KeyFaceSpecs.LANGUAGE);

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

    private static List<KeyFaceLayout> layouts() {
        List<KeyFaceLayout> layouts = new ArrayList<>();
        List<List<KeyboardScheme>> schemeSets = List.of(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.XIAOHE),
            List.of(KeyboardScheme.values()));
        for (int touchLayout = 0; touchLayout <= 9; touchLayout++)
            for (boolean shift : new boolean[] {false, true})
                for (String action : List.of(NineKeyCompositionActionPolicy.SEPARATE, NineKeyCompositionActionPolicy.REWRITE))
                    for (boolean cycles : new boolean[] {false, true})
                        for (List<KeyboardScheme> schemes : schemeSets)
                            layouts.add(layout(touchLayout, shift, action, cycles, schemes));
        return layouts;
    }

    /** 这副键盘上可能的每种此刻状态：选中的方案只取这副键盘可选的（或没有）。 */
    private static List<KeyFaceLive> lives(KeyFaceLayout layout) {
        List<KeyboardScheme> selected = new ArrayList<>(layout.schemes());
        selected.add(null);
        List<KeyFaceLive> lives = new ArrayList<>();
        for (int bits = 0; bits < 128; bits++)
            for (int scheme : new int[] {-1, 0, InputSchemeTraits.WUBI})
                for (KeyboardScheme chosen : selected)
                    lives.add(new KeyFaceLive((bits & 1) != 0, (bits & 2) != 0, (bits & 4) != 0, (bits & 8) != 0,
                        (bits & 16) != 0, (bits & 32) != 0, (bits & 64) != 0, scheme, chosen));
        return lives;
    }

    private static KeyFaceLayout layout(int touchLayout, boolean shift, String action, boolean cycles,
            List<KeyboardScheme> schemes) {
        return new KeyFaceLayout(touchLayout, shift, action, true, ChineseSymbolFaces.Marks.DEFAULT, cycles, schemes,
            List.of());
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
