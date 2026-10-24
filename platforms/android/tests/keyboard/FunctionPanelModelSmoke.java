import app.msime.android.FunctionPanelModel;
import app.msime.android.FunctionPanelModel.Id;
import app.msime.android.FunctionPanelModel.Item;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;

public final class FunctionPanelModelSmoke {
    public static void main(String[] args) {
        check(FunctionPanelModel.pageCount() == 3, "three pages");
        check(labels(0).equals(List.of("全角", "中文标点", "模糊音", "繁体", "手写", "本地输入", "AI 辅助", "设置",
            "单手模式", "浮动键盘", "无痕输入", "键盘高度")),
            "page one");
        check(labels(1).equals(List.of("按键音", "振动", "振动强度", "语音输入", "插入语音文字", "词库", "文本编辑",
            "编辑工具栏", "设备信息", "反馈", "宣传", "打开官网")), "page two");
        check(labels(2).equals(List.of("关于", "表情", "剪贴板历史", "世界时钟", "拍照识字")), "page three");
        List<Item> screenshotItems = FunctionPanelModel.itemsExcept(java.util.Set.of(
            Id.EMOJI, Id.CLIPBOARD, Id.FLOATING, Id.TEXT_EDIT, Id.VOICE_RESULT));
        List<String> screenshotPage = new ArrayList<>();
        for (Item item : screenshotItems.subList(FunctionPanelModel.PAGE_SIZE, screenshotItems.size()))
            screenshotPage.add(item.label());
        check(screenshotPage.equals(List.of("振动", "振动强度", "语音输入", "词库", "编辑工具栏",
            "设备信息", "反馈", "宣传", "打开官网", "关于", "世界时钟", "拍照识字")), "#9191 的第二页顺序");
        check(!FunctionPanelModel.item(Id.TEXT_EDIT).toggle() && FunctionPanelModel.pageOf(Id.TEXT_EDIT) == 1,
            "文本编辑 opens a panel from page two");
        check(FunctionPanelModel.page(3).isEmpty() && FunctionPanelModel.page(-1).isEmpty(),
            "out-of-range pages are empty");
        check(FunctionPanelModel.item(Id.TRADITIONAL).description().equals("繁体输出"),
            "繁体 is described as 繁体输出");
        check(FunctionPanelModel.item(Id.VIBRATION).description().equals("按键振动"),
            "振动 is described as 按键振动");
        check(FunctionPanelModel.item(Id.AI_ASSIST).description().equals("AI 回复与润色")
            && !FunctionPanelModel.item(Id.AI_ASSIST).toggle(), "merged AI tile opens a panel");
        check(FunctionPanelModel.item(Id.WEBSITE).description().equals("打开官网")
            && !FunctionPanelModel.item(Id.WEBSITE).toggle(), "website tile opens the default browser");
        check(FunctionPanelModel.pageOf(Id.LOCAL_INPUT) == 0, "local input sits on page one");
        List<Id> toggles = new ArrayList<>();
        for (Item item : FunctionPanelModel.toggles()) toggles.add(item.id());
        check(toggles.equals(List.of(Id.FULL_WIDTH, Id.CHINESE_PUNCTUATION, Id.FUZZY_PINYIN,
            Id.TRADITIONAL, Id.ONE_HAND, Id.FLOATING, Id.PRIVACY, Id.KEY_SOUND, Id.VIBRATION)), "toggle set");
        check(FunctionPanelModel.item(Id.FLOATING).toggle()
            && FunctionPanelModel.pageOf(Id.FLOATING) == 0, "floating keyboard is a page-one switch (#5621)");
        HashSet<Id> seen = new HashSet<>();
        for (Item item : FunctionPanelModel.items()) check(seen.add(item.id()), "unique ids");
        check(seen.size() == Id.values().length, "every id is placed");
        Item fullWidth = FunctionPanelModel.item(Id.FULL_WIDTH);
        check("已开启".equals(FunctionPanelModel.state(fullWidth, true, true))
            && "已关闭".equals(FunctionPanelModel.state(fullWidth, false, true))
            && "不可用".equals(FunctionPanelModel.state(fullWidth, true, false)), "toggle states");
        check(FunctionPanelModel.state(FunctionPanelModel.item(Id.SETTINGS), false, true) == null,
            "actions carry no on/off state");
        // 所有顶部快捷入口的组合都能去重，未启用的入口仍在菜单里。
        Id[] shortcuts = {Id.EMOJI, Id.CLIPBOARD, Id.FLOATING, Id.TEXT_EDIT, Id.VOICE_INPUT};
        for (int mask = 0; mask < (1 << shortcuts.length); mask++) {
            java.util.Set<Id> toolbar = java.util.EnumSet.noneOf(Id.class);
            for (int index = 0; index < shortcuts.length; index++)
                if ((mask & (1 << index)) != 0) toolbar.add(shortcuts[index]);
            List<Item> visible = FunctionPanelModel.itemsExcept(toolbar);
            HashSet<Id> ids = new HashSet<>();
            for (Item item : visible) check(ids.add(item.id()), "no repeated menu entry");
            for (Id id : shortcuts) check(ids.contains(id) != toolbar.contains(id), "shortcut has exactly one entrance");
            check(ids.contains(Id.SETTINGS) && ids.contains(Id.LOCAL_INPUT) && ids.contains(Id.COPY_DEVICE_INFO)
                && ids.contains(Id.TOOLBAR_EDITOR), "common actions remain accessible");
            check(visible.size() <= 3 * FunctionPanelModel.PAGE_SIZE, "menu stays within three pages");
        }
        List<Item> idle = FunctionPanelModel.itemsExcept(java.util.Set.of(Id.VOICE_RESULT));
        check(idle.stream().anyMatch(item -> item.id() == Id.VOICE_INPUT), "无待取结果时也能直接开始语音输入");
        check(idle.stream().noneMatch(item -> item.id() == Id.VOICE_RESULT), "no empty voice result entrance");
        check(FunctionPanelModel.item(Id.VOICE_RESULT).label().equals("插入语音文字"), "pending voice entry states its action");
        System.out.println("Android function panel: three pages, toolbar deduplication and toggle states passed");
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
