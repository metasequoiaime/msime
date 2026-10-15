package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.os.SystemClock;
import android.os.Build;
import android.graphics.Rect;

/** 在真实输入法菜单中复制设备信息，兼容最低 API 并保留编辑器内容。 */
public final class CopyDeviceInfoDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "keyboard copies device information whitelist without changing editor contents";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "device information editor focus";
        tap(field("msime-test-plain"));
        stage = "device information copy retains editor";
        for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
        await(field("msime-test-plain").and(node -> equalsText("nihao", node.getText())));
        stage = "device information initial commit";
        tap(key("空格"));
        await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
        ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
        ClipData original = clipboard.getPrimaryClip();
        try {
            clipboard.setPrimaryClip(ClipData.newPlainText("synthetic", "MSIME synthetic clipboard sentinel"));
            Rect toolbar = new Rect();
            await(described("键盘快捷栏")).getBoundsInScreen(toolbar);
            stage = "device information menu open";
            tap(key("更多"));
            stage = "device information menu copy";
            if (Build.VERSION.SDK_INT < 30) {
                copyFromLegacyMenu(toolbar);
            } else {
                showTool("复制设备信息");
                tap(tool("复制设备信息"));
            }
            stage = "device information clipboard result";
            String copied = awaitDeviceInfo(clipboard);
            String[] lines = copied.split("\\n");
            String[] labels = {"应用版本", "品牌", "型号", "系统版本", "系统构建", "处理器架构",
                "屏幕分辨率", "存储空间", "运行内存", "Android System WebView", "键盘"};
            if (lines.length != labels.length + 1 || !lines[0].endsWith(" 设备信息"))
                throw new AssertionError("Device information whitelist changed");
            for (int index = 0; index < labels.length; index++)
                if (!lines[index + 1].startsWith(labels[index] + "："))
                    throw new AssertionError("Device information field missing: " + labels[index]);
            if (copied.contains("nihao") || copied.contains("你好") || copied.contains("MSIME synthetic clipboard sentinel")
                || !copied.contains("键盘：已启用 · 当前默认")
                || copied.contains("屏幕分辨率：未知") || copied.contains("应用版本：未知"))
                throw new AssertionError("Device information contains invalid diagnostics");
            stage = "device information copy keeps menu and editor";
            if (Build.VERSION.SDK_INT >= 30) await(toolPanel());
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
            if (Build.VERSION.SDK_INT < 30) {
                // 复制提示会暂时替换工具栏；先刷新节点，等真实工具栏恢复再按品牌键。
                await(described("键盘快捷栏").and(node -> node.refresh() && node.isVisibleToUser()));
                shellDiscard("input tap " + (toolbar.left + toolbar.width() / 12) + " " + toolbar.centerY());
            } else tap(tool("返回键盘"));
            stage = "typing after device information copy";
            for (String letter : new String[] {"s", "h", "i"}) tap(key(letter));
            await(field("msime-test-plain").and(node -> equalsText("你好shi", node.getText())));
            tap(key("空格"));
            await(field("msime-test-plain").and(node -> equalsText("你好是", node.getText())));
        } finally {
            if (original == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(original);
        }
    }

    /**
     * API 28 的 UiAutomation 不提供覆盖在字母键上的菜单节点，改用真实触摸翻页和点击。
     * 专用 AVD 使用默认全拼布局；复制入口是第二页第二行第四列。坐标从当前工具栏边界和密度推导，
     * 不假设设备分辨率。结果仍读取系统剪贴板并验证完整白名单，触摸无效就失败。
     */
    private void copyFromLegacyMenu(Rect toolbar) throws Exception {
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        int firstRow = toolbar.bottom + Math.round(36 * density);
        int right = toolbar.left + toolbar.width() * 7 / 8;
        int left = toolbar.left + toolbar.width() / 8;
        shellDiscard("input swipe " + right + " " + firstRow + " " + left + " " + firstRow + " 320");
        // 网格翻页动画为 260 毫秒，结束后才点击第二行，避免点中滑动中的上一页磁贴。
        SystemClock.sleep(400);
        int secondRow = firstRow + Math.round(68 * density);
        shellDiscard("input tap " + right + " " + secondRow);
    }

    /** 测试编辑器保持前台，按系统剪贴板限制从当前焦点 UID 读取，不输出报告正文。 */
    private String awaitDeviceInfo(ClipboardManager clipboard) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            ClipData clip = clipboard.getPrimaryClip();
            CharSequence text = clip == null || clip.getItemCount() == 0 ? null : clip.getItemAt(0).getText();
            if (text != null && text.toString().contains("\n应用版本：")) return text.toString();
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Keyboard device information copy did not finish");
    }

}
