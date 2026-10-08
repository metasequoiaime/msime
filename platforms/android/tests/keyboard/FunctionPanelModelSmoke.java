import app.msime.android.FunctionPanelModel;
import app.msime.android.FunctionPanelModel.Id;
import app.msime.android.FunctionPanelModel.Item;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;

public final class FunctionPanelModelSmoke {
    public static void main(String[] args) {
        check(FunctionPanelModel.pageCount() == 3, "three pages");
        check(labels(0).equals(List.of("全角", "中文标点", "模糊音", "繁体", "手写", "词库", "键盘高度", "设置")),
            "page one");
        check(labels(1).equals(List.of("按键音", "振动", "单手模式", "隐私模式", "反馈", "关于",
            "AI 回复与润色", "本地输入")), "page two");
        check(labels(2).equals(List.of("语音结果", "振动强度", "表情", "剪贴板历史", "浮动键盘")), "page three");
        check(FunctionPanelModel.page(3).isEmpty() && FunctionPanelModel.page(-1).isEmpty(),
            "out-of-range pages are empty");
        check(FunctionPanelModel.item(Id.TRADITIONAL).description().equals("繁体输出"),
            "繁体 is described as 繁体输出");
        check(FunctionPanelModel.item(Id.VIBRATION).description().equals("按键振动"),
            "振动 is described as 按键振动");
        check(FunctionPanelModel.item(Id.AI_ASSIST).description().equals("AI 回复与润色")
            && !FunctionPanelModel.item(Id.AI_ASSIST).toggle(), "merged AI tile opens a panel");
        check(FunctionPanelModel.pageOf(Id.LOCAL_INPUT) == 1, "local input sits on page two");
        List<Id> toggles = new ArrayList<>();
        for (Item item : FunctionPanelModel.toggles()) toggles.add(item.id());
        check(toggles.equals(List.of(Id.FULL_WIDTH, Id.CHINESE_PUNCTUATION, Id.FUZZY_PINYIN,
            Id.TRADITIONAL, Id.KEY_SOUND, Id.VIBRATION, Id.ONE_HAND, Id.PRIVACY, Id.FLOATING)), "toggle set");
        check(FunctionPanelModel.item(Id.FLOATING).toggle()
            && FunctionPanelModel.pageOf(Id.FLOATING) == 2, "floating keyboard is a page-three switch (#5621)");
        HashSet<Id> seen = new HashSet<>();
        for (Item item : FunctionPanelModel.items()) check(seen.add(item.id()), "unique ids");
        check(seen.size() == Id.values().length, "every id is placed");
        Item fullWidth = FunctionPanelModel.item(Id.FULL_WIDTH);
        check("已开启".equals(FunctionPanelModel.state(fullWidth, true, true))
            && "已关闭".equals(FunctionPanelModel.state(fullWidth, false, true))
            && "不可用".equals(FunctionPanelModel.state(fullWidth, true, false)), "toggle states");
        check(FunctionPanelModel.state(FunctionPanelModel.item(Id.SETTINGS), false, true) == null,
            "actions carry no on/off state");
        System.out.println("Android function panel: P25 three pages, labels, descriptions and toggles passed");
    }

    private static List<String> labels(int page) {
        List<String> result = new ArrayList<>();
        for (Item item : FunctionPanelModel.page(page)) result.add(item.label());
        return result;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
