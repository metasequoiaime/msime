package app.msime.android.home;

import java.util.List;
import java.util.function.Function;

/**
 * 双拼辅助码的触发方式（client-core `ShuangpinHelpcodeModes`，文档里是 `shuangpin_helpcode.modes`，#10624）在「输入 › 辅助码」里的各行：顺序、标题和说明与共享页 `helpcode-page.tsx` 的 modesRows 相同。
 *
 * <p>`factoryDefault` 是 `ShuangpinHelpcodeModes::default()` 的值：这一段等于出厂值时 client-core 不写进文档，缺这一段或缺某个字段时都按它读。互斥（句中间接开着时同一枚触发键归句中、句中间接与逐音节直接不同时开）由 client-core 在写入端归一，页面保存后重读就显示归一后的值，这里不重复这条规则。Tab 手动辅助码（`manual_tab`）只在 macOS 生效，不列。
 *
 * <p>触屏键盘的数字页在中文方案下把分号、反引号和 `/` 按字面上屏（`NumberPadPunctuation.typesThroughSession`），用到它们的那几行在说明里写明要外接键盘；大写字母（Shift 加字母）和直接接码在触屏上照常可用。
 */
public final class HelpcodeModesPolicy {
    /** 一行受哪个总开关管：末尾间接的三枚触发键要末尾直接和逐音节直接都关着，句中三枚触发键随句中间接，两种结束方式随逐音节直接。 */
    public enum Parent { NONE, END_TRIGGER, MID_SENTENCE, DIRECT_HELPCODE }

    /**
     * @param field `modes` 里的字段名
     * @param title 行标题，带「双拼」前缀，与共享页相同
     * @param help 「?」里的说明
     * @param factoryDefault 缺字段时的值
     * @param parent 管它的总开关
     */
    public record Mode(String field, String title, String help, boolean factoryDefault, Parent parent) {}

    private static final String TOUCH_SEMICOLON = "。触屏键盘上的分号直接上屏，这种触发要用外接键盘";
    private static final String TOUCH_BACKTICK = "。触屏键盘上的反引号直接上屏，这种触发要用外接键盘";

    public static final List<Mode> MODES = List.of(
        new Mode("direct", "双拼末尾直接辅助码", "完整音节后直接接辅助码；打开时末尾间接触发不生效", true, Parent.NONE),
        new Mode("trigger_semicolon", "双拼末尾间接触发（分号）",
            "完整音节后按分号，再输入一到两个辅助码字母；微软双拼的分号优先补全韵母" + TOUCH_SEMICOLON, true, Parent.END_TRIGGER),
        new Mode("trigger_backtick", "双拼末尾间接触发（反引号）",
            "完整音节后按反引号，再输入一到两个辅助码字母" + TOUCH_BACKTICK, false, Parent.END_TRIGGER),
        new Mode("trigger_uppercase", "双拼末尾间接触发（大写字母）", "完整音节后输入大写字母作为辅助码", false, Parent.END_TRIGGER),
        new Mode("mid_sentence", "双拼句中间接辅助码", "触发键把它前面的音节标上辅助码，约束带进整句，后面还能接着打", false, Parent.NONE),
        new Mode("mid_sentence_backtick", "双拼句中触发键（反引号）", "用反引号在句中标记辅助码" + TOUCH_BACKTICK, true,
            Parent.MID_SENTENCE),
        new Mode("mid_sentence_semicolon", "双拼句中触发键（分号）",
            "用分号在句中标记辅助码；微软双拼的分号优先补全韵母" + TOUCH_SEMICOLON, false, Parent.MID_SENTENCE),
        new Mode("mid_sentence_uppercase", "双拼句中触发键（大写字母）", "完整音节后输入大写字母在句中标记辅助码", false,
            Parent.MID_SENTENCE),
        new Mode("direct_helpcode", "双拼逐音节直接辅助码", "不要引导键，逐音节接辅码；三码自动识别，四码单独敲时直接定字", false,
            Parent.NONE),
        new Mode("direct_helpcode_slash", "双拼逐音节直接（/ 结束）",
            "用 / 结束一位或两位辅码。触屏键盘上的 / 直接上屏，这种结束方式要用外接键盘", true, Parent.DIRECT_HELPCODE),
        new Mode("direct_helpcode_uppercase", "双拼逐音节直接（第二码大写结束）", "第二位辅码敲大写来结束一段", false,
            Parent.DIRECT_HELPCODE));

    private HelpcodeModesPolicy() {}

    /** 按字段名取一行；字段名不在表里是调用方写错了。 */
    public static Mode mode(String field) {
        for (Mode mode : MODES) {
            if (mode.field().equals(field)) return mode;
        }
        throw new IllegalArgumentException("unknown helpcode mode: " + field);
    }

    /**
     * 这一行此刻的值。
     *
     * @param stored 按字段名取文档里存的值，没有这个字段（或整段 `modes` 都没有）时返回 null
     */
    public static boolean value(Mode mode, Function<String, Boolean> stored) {
        Boolean value = stored.apply(mode.field());
        return value == null ? mode.factoryDefault() : value;
    }

    /** 这一行能不能改（辅助码关着时整组不画，所以这里不看辅助码开关）：有总开关的行要总开关开着，与共享页的 disabled 相同。末尾间接三行照引擎的判据（`apply_request_options` 只在末尾直接和逐音节直接都关着时保留末尾段），只置灰、不改存的值。 */
    public static boolean enabled(Mode mode, boolean direct, boolean midSentence, boolean directHelpcode) {
        return switch (mode.parent()) {
            case END_TRIGGER -> !direct && !directHelpcode;
            case MID_SENTENCE -> midSentence;
            case DIRECT_HELPCODE -> directHelpcode;
            case NONE -> true;
        };
    }
}
