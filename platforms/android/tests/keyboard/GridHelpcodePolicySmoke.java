import app.msime.android.GridHelpcodePolicy;

/** 十四／十八键网格辅助码在键盘上的规则：字母键改送什么、上滑分词键的判定、明确选字面前怎样结束组字、返回键和读屏。 */
public final class GridHelpcodePolicySmoke {
    public static void main(String[] args) {
        // 操作号与 msime_client.h 的 msime_client_grid_helpcode 一致。
        check(GridHelpcodePolicy.BEGIN == 0 && GridHelpcodePolicy.EXACT == 1
            && GridHelpcodePolicy.CLEAR == 2 && GridHelpcodePolicy.GROUP == 3, "操作号与 C 接口一致");

        check(GridHelpcodePolicy.letterOp("", 'a') == GridHelpcodePolicy.NONE, "没有分隔符时字母键照旧走网格键");
        check(GridHelpcodePolicy.letterOp(null, 'W') == GridHelpcodePolicy.NONE, "缺视图时照旧");
        check(GridHelpcodePolicy.letterOp("|", 'a') == GridHelpcodePolicy.GROUP, "放下分隔符后小写送整组");
        check(GridHelpcodePolicy.letterOp("|A", 'W') == GridHelpcodePolicy.EXACT, "横滑的大写送确定字母");

        check(!GridHelpcodePolicy.entrySwipe(0, -24), "阈值内仍是点按");
        check(GridHelpcodePolicy.entrySwipe(0, -25), "上滑过阈值进入");
        check(!GridHelpcodePolicy.entrySwipe(0, 40), "下滑不进入");
        check(!GridHelpcodePolicy.entrySwipe(30, -30), "斜向滑动不进入");
        check(GridHelpcodePolicy.entrySwipe(10, -30), "明显向上允许少量横移");
        check(!GridHelpcodePolicy.entrySwipe(Float.NaN, -40)
            && !GridHelpcodePolicy.entrySwipe(0, Float.NEGATIVE_INFINITY), "无效坐标不进入");

        check(GridHelpcodePolicy.literalPrecommitCommand("", 0) == 2, "没有分隔符照旧原样上屏");
        check(GridHelpcodePolicy.literalPrecommitCommand("|AE", 3) == 2, "有候选时按首选结束");
        check(GridHelpcodePolicy.literalPrecommitCommand("|AE", 0) == 3, "筛空时先取消，不把组码留在字面前面");
        check(GridHelpcodePolicy.literalPrecommitCommand("|", 0) == 3, "只有分隔符而没有候选时同样取消");

        check(GridHelpcodePolicy.backClears("|A", true, false), "第一次返回清辅助码");
        check(!GridHelpcodePolicy.backClears("", true, false), "没有分隔符时照旧收起键盘");
        check(!GridHelpcodePolicy.backClears("|A", false, false), "键盘没显示时不接管");
        check(!GridHelpcodePolicy.backClears("|A", true, true), "面板开着时返回先给面板");

        check(GridHelpcodePolicy.cornerHint(true).equals("辅") && GridHelpcodePolicy.cornerHint(false).isEmpty(),
            "只有能用时画角标");
        check(GridHelpcodePolicy.separatorDescription("分词", true).equals("分词；上滑输入辅助码")
            && GridHelpcodePolicy.separatorDescription("分词", false).equals("分词"), "读屏说明上滑");
        check(GridHelpcodePolicy.choiceDescription("aE").equals("选择辅助码 aE"), "整串解释不读成拼音");
        check(GridHelpcodePolicy.reading("ni'hao", "|[AS]").equals("ni'hao|[AS]")
            && GridHelpcodePolicy.reading("ni'hao", "").equals("ni'hao")
            && GridHelpcodePolicy.reading(null, "|").equals("|"), "读音行接上辅助码显示");
        System.out.println("Android grid helpcode policy: passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
