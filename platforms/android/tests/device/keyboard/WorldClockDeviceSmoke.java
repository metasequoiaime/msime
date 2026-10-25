package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.graphics.Bitmap;
import android.content.ComponentName;
import android.content.Intent;
import android.os.Build;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在原生输入法内离线管理世界时钟，验证真实编辑器上屏和重启持久化。 */
public final class WorldClockDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "offline world clock, city persistence, live updates, multiple insertion formats and long-press deletion";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Test storage escaped sandbox");
        File cities = new File(directory, "WorldClock.json");
        byte[] original = cities.isFile() ? Files.readAllBytes(cities.toPath()) : null;
        try {
            publish(cities, new JSONObject().put("cities", new JSONArray()).toString().getBytes(StandardCharsets.UTF_8));
            editor();
            openClock("initial display");
            checkpoint("system clock follows the current system date and time");
            AccessibilityNodeInfo system = await(described("系统时间时钟"));
            String initial = system.getText().toString();
            if (!initial.contains("UTC") || !initial.contains("年") || !initial.contains("星期")
                    || !initial.contains("日出") || initial.contains("null")) throw new AssertionError("System clock is incomplete");
            SystemClock.sleep(2200);
            dropStaleNodes();
            await(described("系统时间时钟").and(node -> node.refresh() && !initial.equals(node.getText().toString())));
            checkpoint("add Tokyo offline through real city controls");
            addCity("亚洲", "日本", "Asia/Tokyo");
            awaitAny(described("东京时钟"));
            checkpoint("add New York offline through real city controls");
            addCity("美洲", "美国", "America/New_York");
            awaitAny(described("纽约时钟"));
            if (read(cities).getJSONArray("cities").length() != 2) throw new AssertionError("City selection was not saved");
            checkpoint("world clock pixels");
            Bitmap screenshot = automation.takeScreenshot();
            if (screenshot == null) throw new AssertionError("Clock screenshot unavailable");
            try (FileOutputStream output = new FileOutputStream(new File(getTargetContext().getCacheDir(), "world-clock-synthetic.png"))) {
                screenshot.compress(Bitmap.CompressFormat.PNG, 100, output);
            } finally { screenshot.recycle(); }
            checkpoint("insert Tokyo ISO time into the real editor");
            tap(described("东京时钟"));
            tap(format("ISO 8601"));
            dropStaleNodes();
            await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().matches("\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}\\+09:00")));
            checkpoint("saved cities survive IME restart");
            editor();
            openClock("after persistence restart");
            awaitAny(described("东京时钟"));
            awaitAny(described("纽约时钟"));
            checkpoint("insert weekday from the system clock");
            tap(described("系统时间时钟"));
            tap(format("星期"));
            dropStaleNodes();
            await(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
                && node.getText().toString().matches(".*星期[一二三四五六日]")));
            openClock("after weekday insertion");
            checkpoint("long-press city and delete");
            AccessibilityNodeInfo tokyo = awaitAny(described("东京时钟"));
            tokyo.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
            SystemClock.sleep(300);
            tokyo = await(described("东京时钟"));
            if (!tokyo.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK)) throw new AssertionError("City does not support long press");
            tap(node -> equalsText("删除东京", node.getText()));
            awaitAny(described("系统时间时钟"));
            long deadline = SystemClock.uptimeMillis() + 8000;
            while (read(cities).getJSONArray("cities").length() != 1 && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(100);
            JSONArray saved = read(cities).getJSONArray("cities");
            if (saved.length() != 1 || !saved.getString(0).equals("America/New_York")) throw new AssertionError("Deletion lost the wrong city");
            editor();
            openClock("after deletion restart");
            awaitAny(described("纽约时钟"));
            checkpoint("return to keyboard after clock");
            tap(node -> equalsText("返回", node.getText()) && equalsText("app.msime.android", node.getPackageName()));
            showTool("世界时钟");
            tap(key("更多"));
            awaitAny(keyboardLetter());
        } finally {
            leaveEditor();
            if (original == null) Files.deleteIfExists(cities.toPath()); else publish(cities, original);
            shellDiscard("ime set app.msime.android/.MSIMEInputService");
        }
    }

    @SuppressWarnings("deprecation")
    private void editor() throws Exception {
        // 先用系统 Home 收起键盘再重启输入法，不启动与时钟无关的设置首页。
        checkpoint("leave clock and restart IME");
        leaveEditor();
        restartClockIme();
        // 输入法已停止时钟刷新；等新编辑器完成冷启动，再按节点确认焦点。
        checkpoint("launch fresh synthetic editor");
        automation.executeAndWaitForEvent(() -> {
            getContext().startActivity(new Intent().setComponent(new ComponentName(
                    "app.msime.android.test", "app.msime.android.test.EditorActivity"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        }, event -> event.getEventType() == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED
                && equalsText("app.msime.android.test", event.getPackageName()), 45000).recycle();
        checkpoint("world clock editor focus");
        // 窗口事件可能先于可访问树到达；冷启动只等待准备条件，不重试功能断言。
        long deadline = SystemClock.uptimeMillis() + 60000;
        while (!editorReady()) {
            if (SystemClock.uptimeMillis() >= deadline) {
                Bitmap screenshot = automation.takeScreenshot();
                if (screenshot != null) {
                    try (FileOutputStream output = new FileOutputStream(new File(
                            getTargetContext().getCacheDir(), "world-clock-editor-not-ready.png"))) {
                        screenshot.compress(Bitmap.CompressFormat.PNG, 100, output);
                    } finally { screenshot.recycle(); }
                }
                throw new AssertionError("Synthetic editor accessibility window did not become ready");
            }
            SystemClock.sleep(100);
        }
        tap(field("msime-test-plain"));
        await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        if (Build.VERSION.SDK_INT >= 30) {
            checkpoint("editor IME display requests settled");
            awaitAny(field("msime-test-plain").and(node -> node.refresh()
                    && equalsText("ime-requests-settled", node.getStateDescription())));
        }
    }

    private boolean editorReady() {
        // 输入法也可能成为活动窗口；编辑器准备状态应从全部交互窗口判断。
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), field("msime-test-plain")) != null) return true;
        }
        return false;
    }

    private void leaveEditor() {
        if (!automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME))
            throw new AssertionError("System Home action failed");
    }

    private void restartClockIme() throws Exception {
        // 只重试系统输入法注册尚未稳定的准备步骤，功能断言失败仍立即报告。
        for (int attempt = 1; ; attempt++) {
            try {
                restartIme();
                return;
            } catch (AssertionError failure) {
                if (attempt == 3 || !"Test input method selection did not settle after restart".equals(failure.getMessage())) throw failure;
                checkpoint("retry IME registration setup " + attempt);
            }
        }
    }

    private void openClock(String purpose) throws Exception {
        // 编辑器收到提交回声时，输入法还可能正在收起格式页；等键区恢复再重新展开菜单。
        checkpoint(purpose + ": keyboard restored");
        dropStaleNodes();
        AccessibilityNodeInfo letter = awaitAny(keyboardLetter());
        checkpoint(purpose + ": keyboard control " + letter.getClassName()
                + " / " + letter.getContentDescription());
        checkpoint(purpose + ": expand function menu");
        tap(key("更多"));
        dropStaleNodes();
        awaitAny(toolPanel().and(AccessibilityNodeInfo::refresh));
        checkpoint(purpose + ": locate world clock entry");
        showTool("世界时钟");
        tap(tool("世界时钟"));
        checkpoint(purpose + ": system clock visible");
        await(described("系统时间时钟"));
    }

    private void checkpoint(String value) {
        stage = value;
        android.os.Bundle progress = new android.os.Bundle();
        progress.putString("stream", "WORLD_CLOCK_STAGE: " + value + "\n");
        sendStatus(0, progress);
    }

    private Predicate<AccessibilityNodeInfo> keyboardLetter() {
        // 先刷新再判断可见性；通用 await 在刷新谓词之前会跳过缓存中仍不可见的节点。
        return node -> equalsText("app.msime.android", node.getPackageName())
                && node.getText() != null && "q".equalsIgnoreCase(node.getText().toString())
                && node.refresh() && node.isVisibleToUser() && node.isClickable();
    }

    private void addCity(String region, String country, String zone) throws Exception {
        tap(node -> equalsText("添加城市", node.getText()));
        tap(node -> equalsText(region, node.getText()));
        tap(node -> equalsText(country, node.getText()));
        tap(node -> node.getText() != null && node.getText().toString().contains(" · " + zone));
        awaitAny(described("系统时间时钟"));
    }

    private static Predicate<AccessibilityNodeInfo> format(String label) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.getText() != null
            && node.getText().toString().startsWith(label + "\n");
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
}
