package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.function.Predicate;

/** 真机验收功能菜单的排列、状态、子页和返回键盘。 */
public final class MoreToolsDeviceSmoke extends DeviceSmoke {
    /** 功能面板的固定尾部（#9945，见 `FunctionPanelModel`）：新增工具只能排在它们前面。 */
    private static final List<String> FIXED_TAIL = List.of("设置", "反馈", "宣传", "打开官网", "关于");

    @Override protected String successDescription() {
        return "toolbar row, four-column function panel, local-input subpanel and keyboard return; hidden toolbar scheme entry opens the picker and switches layouts";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "more tools focus";
        tap(field("msime-test-plain"));
        stage = "keyboard shortcut bar";
        await(shortcutBar());
        // 方案入口的描述带当前方案名，只按前缀识别；其余入口使用固定标签。
        await(describedPrefix("输入方案"));
        for (String label : new String[] {"更多", "皮肤", "收起"})
            await(key(label));
        await(described("表情"));
        await(described("常用语"));
        stage = "more tools open";
        tap(key("更多"));
        AccessibilityNodeInfo panel = await(toolPanel());
        await(described("返回键盘"));
        stage = "function panel first page";
        // 前一个套件可能在第二页复制设备信息，复用磁贴后页码会保留。
        showTool("全角");
        // 默认工具栏已有输入方式、表情和剪贴板，菜单不重复它们；#9945 把设置移进固定尾部后，第一页末格由按键音补上。
        String[] firstPage = {"全角", "中文标点", "模糊音", "繁体输出", "手写", "本地输入", "AI 回复与润色", "单手模式",
            "浮动键盘", "无痕输入", "键盘调整", "按键音"};
        // 面板打开时磁贴有一段展开动画（同一磁贴先后量到过 73、109 像素宽），等它停下再量。
        awaitStableBounds(tool(firstPage[0]));
        panel = await(toolPanel());
        Rect panelBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
        AccessibilityNodeInfo[] tiles = new AccessibilityNodeInfo[firstPage.length];
        int expectedHeight = Math.round(52 * getTargetContext()
            .getResources().getDisplayMetrics().density);
        for (int index = 0; index < firstPage.length; index++) {
            AccessibilityNodeInfo tile = await(tool(firstPage[index]));
            tiles[index] = tile;
            Rect bounds = new Rect();
            tile.getBoundsInScreen(bounds);
            if (bounds.width() <= panelBounds.width() / 6 || bounds.width() >= panelBounds.width() / 3)
                throw new AssertionError("Function panel tile is not one of four columns: " + firstPage[index]
                    + " tile " + bounds.toShortString() + " panel " + panelBounds.toShortString());
            if (Math.abs(bounds.height() - expectedHeight) > 2)
                throw new AssertionError("Function panel tile height mismatch: " + firstPage[index]
                    + " height " + bounds.height() + " expected " + expectedHeight);
        }
        assertSameRow(tiles[0], tiles[3], "first row");
        assertSameRow(tiles[4], tiles[7], "second row");
        assertSameRow(tiles[8], tiles[11], "third row");
        for (String toggle : new String[] {"全角", "中文标点", "模糊音", "繁体输出"})
            if (!validSettingState(await(tool(toggle))))
                throw new AssertionError("Toggle state was not exposed: " + toggle);
        stage = "function panel later pages";
        // 按键音在第一页末格，按键振动和其余工具在后面几页；页码随工具栏显示哪些入口而变，所以逐个翻到所在页再读。
        if (!validFeedbackState(showTool("按键音")) || !validFeedbackState(showTool("按键振动")))
            throw new AssertionError("Feedback state was not exposed");
        for (String title : new String[] {"反馈", "关于", "打开官网", "词库", "文本编辑", "设备信息"})
            showTool(title);
        stage = "功能菜单辅助入口按截图顺序排列";
        // #9945 起设置、反馈、宣传、打开官网、关于固定在全部工具之后，默认工具栏下跨两页，只能按翻页顺序比较，同屏坐标只比得了同一页。
        List<String> order = functionPanelOrder();
        int previous = -1;
        for (String title : new String[] {"编辑工具栏", "设备信息", "设置", "反馈", "宣传", "打开官网", "关于"}) {
            int index = order.indexOf(title);
            if (index <= previous) throw new AssertionError("功能菜单辅助入口顺序错误：" + title + " " + order);
            previous = index;
        }
        if (!order.subList(order.size() - FIXED_TAIL.size(), order.size()).equals(FIXED_TAIL))
            throw new AssertionError("功能菜单固定尾部之后还有条目：" + order);
        AccessibilityNodeInfo sound = showTool("按键音");
        String originalSound = switchState(sound).toString();
        String changedSound = "已开启".equals(originalSound) ? "已关闭" : "已开启";
        stage = "more tools sound update";
        tap(tool("按键音"));
        await(toolWithState("按键音", changedSound));
        tap(tool("按键音"));
        await(toolWithState("按键音", originalSound));
        stage = "function panel strength";
        AccessibilityNodeInfo strength = showTool("振动强度");
        if (!validStrengthCard(strength)) throw new AssertionError("Strength tile lost its level");
        stage = "more tools local input subpanel";
        showTool("本地输入");
        tap(tool("本地输入"));
        await(tool("返回工具"));
        AccessibilityNodeInfo unicode = await(tool("Unicode 码点"));
        AccessibilityNodeInfo date = await(tool("日期时间"));
        AccessibilityNodeInfo initials = await(tool("超级简拼"));
        assertSameRow(unicode, initials, "local input first row");
        Rect unicodeBounds = new Rect();
        Rect dateBounds = new Rect();
        unicode.getBoundsInScreen(unicodeBounds);
        date.getBoundsInScreen(dateBounds);
        if (unicodeBounds.left >= dateBounds.left || unicodeBounds.right > dateBounds.left)
            throw new AssertionError("Local input entries overlap");
        Rect backBounds = new Rect();
        await(tool("返回工具")).getBoundsInScreen(backBounds);
        if (backBounds.width() >= unicodeBounds.width())
            throw new AssertionError("Subpage back action still occupies a full row");
        tap(tool("返回工具"));
        stage = "AI chooser entries and compact back";
        showTool("AI 回复与润色");
        tap(tool("AI 回复与润色"));
        AccessibilityNodeInfo reply = await(tool("生成高情商回复"));
        AccessibilityNodeInfo polish = await(tool("打开 AI 润色"));
        assertSameRow(reply, polish, "AI actions");
        tap(tool("返回工具"));
        showTool("全角");
        stage = "more tools return";
        tap(tool("返回键盘"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
        checkHiddenSchemeShortcut();
    }

    /** 通过原生设置隐藏工具栏入口，再从品牌菜单切换方案，最后恢复原方案与开关。 */
    private void checkHiddenSchemeShortcut() throws Exception {
        String originalScheme = await(describedPrefix("输入方案：")).getContentDescription()
            .toString().substring("输入方案：".length());
        try {
            setSchemeShortcut(false);
            openEditor();
            if (visible(describedPrefix("输入方案：")))
                throw new AssertionError("关闭的输入方式仍显示在工具栏");
            stage = "隐藏工具栏入口后从菜单打开输入方式";
            tap(key("更多"));
            showTool("全角");
            tap(tool("输入方式"));
            await(described("输入方案卡片区域"));
            if (visible(toolPanel())) throw new AssertionError("输入方式面板仍被功能菜单覆盖");
            tap(described("输入方案卡片 英文 26 键"));
            await(key("q"));
            stage = "菜单切换后再次打开并恢复原输入方案";
            tap(key("更多"));
            tap(tool("输入方式"));
            await(described("输入方案卡片 英文 26 键").and(node ->
                equalsText("已选中", node.getStateDescription())));
            tap(described("输入方案卡片 " + originalScheme));
            await(key("n"));
            // 收起面板应回到字母键盘，不能回到已关闭的功能菜单。
            tap(key("更多"));
            tap(tool("输入方式"));
            await(described("输入方案卡片 " + originalScheme).and(node ->
                equalsText("已选中", node.getStateDescription())));
            tap(described("返回键盘"));
            await(key("n"));
            if (visible(toolPanel())) throw new AssertionError("返回键盘后功能菜单仍可见");
        } finally {
            setSchemeShortcut(true);
            openEditor();
        }
        stage = "重新显示工具栏入口后菜单去重";
        await(describedPrefix("输入方案："));
        tap(key("更多"));
        showTool("全角");
        if (visible(tool("输入方式"))) throw new AssertionError("工具栏与菜单重复显示输入方式");
        tap(described("返回键盘"));
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(shortcutBar());
    }

    private void setSchemeShortcut(boolean enabled) throws Exception {
        stage = enabled ? "恢复输入方式工具栏入口" : "关闭输入方式工具栏入口";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TOOLBAR_EDITOR");
        Predicate<AccessibilityNodeInfo> toggle = node -> node.isCheckable() && containsLabel(node, "输入方式");
        await(node -> equalsText("编辑工具栏", node.getText()));
        AccessibilityNodeInfo node = awaitAny(toggle);
        node.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        node = await(toggle);
        if (node.isChecked() != enabled) tap(toggle);
        await(toggle.and(item -> item.isChecked() == enabled));
    }

    private boolean containsLabel(AccessibilityNodeInfo node, String label) {
        if (equalsText(label, node.getText())) return true;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo child = node.getChild(index);
            if (child != null && containsLabel(child, label)) return true;
        }
        return false;
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        dropStaleNodes();
        for (var window : automation.getWindows())
            if (find(window.getRoot(), match.and(AccessibilityNodeInfo::isVisibleToUser)) != null) return true;
        return false;
    }

    /** 从第一页翻到最后一页，按页、行、列读出功能菜单条目的描述。 */
    private List<String> functionPanelOrder() {
        Predicate<AccessibilityNodeInfo> grid = node -> equalsText("app.msime.android", node.getPackageName())
            && node.isScrollable() && node.getParent() != null
            && equalsText("更多工具", node.getParent().getContentDescription());
        while (awaitAny(grid).performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)) settledPage(grid);
        List<String> order = new ArrayList<>();
        while (true) {
            order.addAll(settledPage(grid));
            if (!awaitAny(grid).performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)) return order;
        }
    }

    /** 当前页看得见的条目描述，按行、列排；翻页有动画，连续两次读到同样的条目和位置才算停下。 */
    private List<String> settledPage(Predicate<AccessibilityNodeInfo> grid) {
        String previous = null;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            dropStaleNodes();
            AccessibilityNodeInfo node = awaitAny(grid);
            List<AccessibilityNodeInfo> tiles = new ArrayList<>();
            for (int index = 0; index < node.getChildCount(); index++) {
                AccessibilityNodeInfo child = node.getChild(index);
                if (child != null && child.isVisibleToUser() && child.getContentDescription() != null) tiles.add(child);
            }
            tiles.sort(Comparator.comparingInt((AccessibilityNodeInfo tile) -> bounds(tile).top)
                .thenComparingInt(tile -> bounds(tile).left));
            List<String> labels = new ArrayList<>();
            StringBuilder signature = new StringBuilder();
            for (AccessibilityNodeInfo tile : tiles) {
                labels.add(tile.getContentDescription().toString());
                signature.append(tile.getContentDescription()).append(bounds(tile).toShortString());
            }
            if (!labels.isEmpty() && signature.toString().equals(previous)) return labels;
            previous = signature.toString();
            SystemClock.sleep(250);
        }
        throw new AssertionError("Function panel page never settled; IME showed " + imeTexts());
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds;
    }

    private void assertSameRow(AccessibilityNodeInfo first, AccessibilityNodeInfo second,
                               String description) {
        Rect firstBounds = new Rect();
        Rect secondBounds = new Rect();
        first.getBoundsInScreen(firstBounds);
        second.getBoundsInScreen(secondBounds);
        if (Math.abs(firstBounds.top - secondBounds.top) > 2)
            throw new AssertionError(description + " is not horizontal");
    }

    private boolean validFeedbackState(AccessibilityNodeInfo node) {
        CharSequence state = switchState(node);
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state))
            && node.isSelected() == equalsText("已开启", state);
    }

    private boolean validSettingState(AccessibilityNodeInfo node) {
        CharSequence state = switchState(node);
        return state != null && (equalsText("已开启", state) || equalsText("已关闭", state)
            || equalsText("不可用", state));
    }

    /** 磁贴文字是「振动强度 」加当前档：轻、中、强或系统（跟随系统）。「强度」里本来就有「强」，所以只看后面那一截。 */
    private boolean validStrengthCard(AccessibilityNodeInfo node) {
        CharSequence text = node.getText();
        if (text == null || !text.toString().startsWith("振动强度 ")) return false;
        String level = text.toString().substring("振动强度 ".length());
        return level.equals("轻") || level.equals("中") || level.equals("强") || level.equals("系统");
    }

    private Predicate<AccessibilityNodeInfo> shortcutBar() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("键盘快捷栏", node.getContentDescription());
    }

}
