package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.Set;
import java.util.function.Predicate;

/** 从独立编辑器 UID 触摸键盘入口，跨过图标组件延迟广播后页面仍可用。 */
public final class HostLaunchDeviceSmoke extends DeviceSmoke {
    private String product;
    private String style;

    @Override public void onCreate(Bundle args) {
        style = args == null ? "CLASSIC" : args.getString("style", "CLASSIC");
        product = args == null ? "app.msime.android" : args.getString("product", "app.msime.android");
        super.onCreate(args);
    }

    @Override protected String successDescription() {
        return "icon keyboard routes survive delayed broadcast and reach settings, lexicon, feedback and about";
    }

    @Override protected void runChecks() throws Exception {
        if (!Set.of("CLASSIC", "FOREST", "SKY", "DUSK", "VERMILION").contains(style))
            throw new AssertionError("Unknown test style");
        if (!product.equals("app.msime.android") && !product.equals("app.msime.android.pinyin"))
            throw new AssertionError("Unknown test product");
        String ime = product.equals("app.msime.android")
            ? "app.msime.android/.MSIMEInputService"
            : "app.msime.android.pinyin/app.msime.android.MSIMEInputService";
        restartIme(ime);
        openEditor();
        for (String route : new String[]{"设置", "词库", "反馈"}) {
            stage = style + " " + route + " route";
            tap(key("更多"));
            showHostTool(route);
            tap(described(route));
            String marker = route.equals("设置") ? "试用键盘" : route.equals("词库") ? "已安装" : "帮助与反馈";
            await(hostText(marker));
            if (route.equals("设置")) {
                stage = style + " settings survives delayed component change";
                long deadline = SystemClock.uptimeMillis() + 15_000;
                while (SystemClock.uptimeMillis() < deadline) {
                    await(hostText(marker));
                    SystemClock.sleep(250);
                }
            }
            openEditor();
        }
        stage = style + " about route";
        tap(key("更多"));
        showHostTool("关于");
        tap(described("关于"));
        await(node -> equalsText(product, node.getPackageName()) && node.getText() != null
            && node.getText().toString().startsWith("版本 "));
    }

    private Predicate<AccessibilityNodeInfo> hostText(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText(text, node.getText());
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(key("n"));
    }

    private void showHostTool(String tool) {
        for (int attempt = 0; attempt < 4; attempt++) {
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (find(window.getRoot(), described(tool)) != null) return;
            }
            AccessibilityNodeInfo grid = awaitAny(node -> equalsText(product, node.getPackageName()) && node.isScrollable());
            grid.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(350);
        }
        throw new AssertionError("Host tool unavailable");
    }

    @Override protected Predicate<AccessibilityNodeInfo> key(String text) {
        return node -> equalsText(product, node.getPackageName()) && node.getText() != null
            && text.equalsIgnoreCase(node.getText().toString());
    }

    @Override protected Predicate<AccessibilityNodeInfo> described(String text) {
        return node -> equalsText(product, node.getPackageName()) && equalsText(text, node.getContentDescription());
    }

    // API 28 的窗口节点缓存会在切换页面后过期；刷新后才按可见性查找。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }
}
