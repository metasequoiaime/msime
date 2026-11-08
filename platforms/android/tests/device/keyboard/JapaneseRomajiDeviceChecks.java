package app.msime.android.test;

import android.graphics.Bitmap;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.util.function.Predicate;

/** 经产品键盘与真实编辑器验证长音留在日语读音里，不直接上屏减号。 */
final class JapaneseRomajiDeviceChecks {
    private JapaneseRomajiDeviceChecks() { }

    static void run(DeviceSmoke test, boolean before) throws Exception {
        Predicate<AccessibilityNodeInfo> longVowel = test.key("ー")
            .and(node -> "android.widget.Button".contentEquals(node.getClassName()));
        test.stage = "日语罗马字键盘就绪";
        test.await(test.key("l"));
        test.await(test.key("空白"));
        // 两版都先完成相同的组字与清空，使截图键面处于同一个稳定状态。
        test.tap(test.key("k"));
        test.tap(test.key("o"));
        awaitText(test, "こ");
        test.tap(test.key("⌫"));
        test.tap(test.key("⌫"));
        test.await(test.field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        if (before) {
            for (var window : test.automation.getWindows()) {
                if (test.find(window.getRoot(), longVowel) != null)
                    throw new AssertionError("修改前的键盘不应已有独立长音键");
            }
            screenshot(test, "japanese-romaji-before.png");
            return;
        }
        test.await(longVowel);
        screenshot(test, "japanese-romaji-after.png");
        test.stage = "长音接入连续读音";
        for (String key : new String[] {"k", "o", "ー", "h", "i", "ー"})
            test.tap(key.equals("ー") ? longVowel : test.key(key));
        awaitText(test, "こーひー");
        screenshot(test, "japanese-romaji-composing.png");
        test.stage = "删除长音后继续组词";
        test.tap(test.key("⌫"));
        awaitText(test, "こーひ");
        test.tap(longVowel);
        awaitText(test, "こーひー");
        // 罗马字预编辑共六个键；全部撤回后单独输入长音，避免依赖词典排序。
        for (int index = 0; index < 6; index++) test.tap(test.key("⌫"));
        test.await(test.field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        test.stage = "单独输入并提交长音";
        test.tap(longVowel);
        awaitText(test, "ー");
        test.tap(test.key("確定"));
        awaitText(test, "ー");
        screenshot(test, "japanese-romaji-committed.png");
    }

    private static void awaitText(DeviceSmoke test, String expected) throws Exception {
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null
            && expected.contentEquals(node.getText())));
    }

    /** 只截专用模拟器里的合成编辑器，保留原始尺寸和真实系统绘制。 */
    private static void screenshot(DeviceSmoke test, String name) throws Exception {
        test.automation.waitForIdle(500, 5000);
        SystemClock.sleep(200);
        Bitmap screen = test.automation.takeScreenshot();
        if (screen == null) throw new AssertionError("模拟器截图不可用");
        try (var output = new FileOutputStream(new File(test.getTargetContext().getFilesDir(), name))) {
            if (!screen.compress(Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("模拟器截图保存失败");
        } finally {
            screen.recycle();
        }
    }
}
