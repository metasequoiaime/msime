package app.msime.android.test;

import android.graphics.Rect;
import android.os.Build;

/** 通过真实长按和菜单进入原生工具栏编辑页，已有键盘设置页不能吞掉跳转。 */
public final class ToolbarEditorDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return Build.VERSION.SDK_INT < 30
            ? "physical brand and skin long presses open toolbar editor; native order sheet cancellation"
            : "physical brand and skin long presses, menu entry, existing settings route and native order sheet cancellation";
    }

    @Override protected void runChecks() throws Exception {
        stage = "existing keyboard settings page";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        await(node -> equalsText("英文自动大写", node.getText()));

        stage = "physical brand long press";
        openKeyboard();
        longPress("更多");
        awaitToolbarEditor();

        stage = "native toolbar order sheet";
        tap(node -> equalsText("按钮顺序", node.getText()));
        await(described("保存，保存按钮顺序"));
        stage = "cancel toolbar order sheet";
        tap(described("取消，放弃本次调整"));
        awaitToolbarEditor();

        stage = "physical skin long press";
        openKeyboard();
        longPress("皮肤");
        awaitToolbarEditor();

        // API 28 的系统无障碍树不暴露功能面板；该系统仍覆盖真实长按与原生设置。
        if (Build.VERSION.SDK_INT >= 30) {
            stage = "menu toolbar editor entry";
            openKeyboard();
            tap(key("更多"));
            showTool("编辑工具栏");
            tap(tool("编辑工具栏"));
            awaitToolbarEditor();
        }
    }

    private void openKeyboard() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(described("键盘快捷栏"));
    }

    private void longPress(String label) throws Exception {
        Rect bounds = new Rect();
        awaitStableBounds(key(label)).getBoundsInScreen(bounds);
        shellDiscard("input swipe " + bounds.centerX() + " " + bounds.centerY()
            + " " + bounds.centerX() + " " + bounds.centerY() + " 800");
    }

    private void awaitToolbarEditor() {
        await(node -> equalsText("编辑工具栏", node.getText()));
        await(node -> equalsText("按钮顺序", node.getText()));
        // 分类切换前没有布局项，证明首次到达的不是键盘页默认的「布局」。
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), node -> node.isVisibleToUser()
                    && equalsText("英文自动大写", node.getText())) != null)
                throw new AssertionError("Toolbar editor opened the layout section");
        }
    }
}
