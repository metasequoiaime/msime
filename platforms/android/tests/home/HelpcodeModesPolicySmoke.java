import app.msime.android.home.HelpcodeModesPolicy;
import app.msime.android.home.HelpcodeModesPolicy.Mode;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 「输入 › 辅助码」里双拼辅助码触发方式（`shuangpin_helpcode.modes`，#10624）的各行：顺序和文案照共享页 modesRows，缺字段时按 client-core 的 `ShuangpinHelpcodeModes::default()` 读，哪几行随总开关置灰，以及触屏上按不到的触发键在说明里写明要外接键盘。读写偏好的那一半在 `TypingPage` 里，要 org.json，这里跑不了。
 */
public final class HelpcodeModesPolicySmoke {
    public static void main(String[] arguments) {
        List<String> order = List.of("direct", "trigger_semicolon", "trigger_backtick", "trigger_uppercase",
            "mid_sentence", "mid_sentence_backtick", "mid_sentence_semicolon", "mid_sentence_uppercase",
            "direct_helpcode", "direct_helpcode_slash", "direct_helpcode_uppercase");
        check(HelpcodeModesPolicy.MODES.size() == order.size(), "eleven modes");
        for (int index = 0; index < order.size(); index++) {
            check(HelpcodeModesPolicy.MODES.get(index).field().equals(order.get(index)), "modes follow the shared page order");
        }
        for (Mode mode : HelpcodeModesPolicy.MODES) {
            check(!mode.field().equals("manual_tab"), "manual_tab stays macOS only");
            check(mode.title().startsWith("双拼") && !mode.help().isEmpty(), "every row has a title and an explanation");
        }

        // 缺字段时等于 client-core 的出厂值：末尾直接、末尾分号、句中反引号、逐音节 / 结束开，其余关。
        Set<String> onByDefault = Set.of("direct", "trigger_semicolon", "mid_sentence_backtick", "direct_helpcode_slash");
        Map<String, Boolean> missing = new HashMap<>();
        for (Mode mode : HelpcodeModesPolicy.MODES) {
            check(mode.factoryDefault() == onByDefault.contains(mode.field()), "fallback equals the client-core default: " + mode.field());
            check(HelpcodeModesPolicy.value(mode, missing::get) == onByDefault.contains(mode.field()), "a missing field reads as the default");
        }
        Map<String, Boolean> stored = Map.of("direct", false, "mid_sentence", true);
        check(!HelpcodeModesPolicy.value(HelpcodeModesPolicy.mode("direct"), stored::get), "a stored value wins over the default");
        check(HelpcodeModesPolicy.value(HelpcodeModesPolicy.mode("mid_sentence"), stored::get), "a stored true is read");

        // 标题与共享页逐字相同。
        check(HelpcodeModesPolicy.mode("direct").title().equals("双拼末尾直接辅助码"), "direct title");
        check(HelpcodeModesPolicy.mode("trigger_uppercase").title().equals("双拼末尾间接触发（大写字母）"), "uppercase trigger title");
        check(HelpcodeModesPolicy.mode("mid_sentence").title().equals("双拼句中间接辅助码"), "mid-sentence title");
        check(HelpcodeModesPolicy.mode("direct_helpcode_slash").title().equals("双拼逐音节直接（/ 结束）"), "slash title");

        // 置灰：末尾间接三枚触发键要末尾直接和逐音节直接都关着，句中三枚触发键随句中间接，两种结束方式随逐音节直接，其余总能改（辅助码关着时整组不画）。参数依次是末尾直接、句中间接、逐音节直接。
        for (String field : List.of("direct", "mid_sentence", "direct_helpcode")) {
            check(HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), true, false, true), "top-level rows are always editable: " + field);
            check(HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), false, false, false), "top-level rows are always editable: " + field);
        }
        // 引擎只在末尾直接和逐音节直接都关着时认末尾间接触发键（`apply_request_options`），出厂值的末尾直接开着，所以这三行出厂时置灰。
        for (String field : List.of("trigger_semicolon", "trigger_backtick", "trigger_uppercase")) {
            Mode mode = HelpcodeModesPolicy.mode(field);
            check(mode.parent() == HelpcodeModesPolicy.Parent.END_TRIGGER, "end triggers hang off direct and direct_helpcode: " + field);
            check(!HelpcodeModesPolicy.enabled(mode, true, false, false), "end triggers are greyed while direct is on: " + field);
            check(!HelpcodeModesPolicy.enabled(mode, false, false, true), "end triggers are greyed while direct_helpcode is on: " + field);
            check(!HelpcodeModesPolicy.enabled(mode, true, false, true), "end triggers are greyed while both are on: " + field);
            check(HelpcodeModesPolicy.enabled(mode, false, false, false), "end triggers open when both are off: " + field);
            check(HelpcodeModesPolicy.enabled(mode, false, true, false), "mid_sentence does not grey the end triggers: " + field);
        }
        check(HelpcodeModesPolicy.mode("direct").help().equals("完整音节后直接接辅助码；打开时末尾间接触发不生效"),
            "the direct row says it turns the end triggers off");
        for (String field : List.of("mid_sentence_backtick", "mid_sentence_semicolon", "mid_sentence_uppercase")) {
            check(!HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), false, false, true), "mid-sentence keys follow mid_sentence");
            check(HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), true, true, false), "mid-sentence keys open with mid_sentence");
        }
        for (String field : List.of("direct_helpcode_slash", "direct_helpcode_uppercase")) {
            check(!HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), false, true, false), "terminators follow direct_helpcode");
            check(HelpcodeModesPolicy.enabled(HelpcodeModesPolicy.mode(field), true, false, true), "terminators open with direct_helpcode");
        }

        // 触屏键盘把分号、反引号和 / 按字面上屏，用到它们的那几行在说明里写明要外接键盘；大写字母和直接接码在触屏上能用，不加。
        Set<String> hardwareOnly = Set.of("trigger_semicolon", "trigger_backtick", "mid_sentence_backtick",
            "mid_sentence_semicolon", "direct_helpcode_slash");
        for (Mode mode : HelpcodeModesPolicy.MODES) {
            check(mode.help().contains("外接键盘") == hardwareOnly.contains(mode.field()), "touch caveat only on unreachable keys: " + mode.field());
        }

        System.out.println("HelpcodeModesPolicySmoke OK");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
