package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在系统输入法里长按原空格，不用拆离窗口的假视图代替命中与触摸流验收。 */
final class VoiceHoldDeviceChecks {
    private final DeviceSmoke t;
    private long downAt;
    private float x;
    private float y;
    private boolean touching;

    VoiceHoldDeviceChecks(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        File preferences = t.sharedPreferencesFile();
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        try {
            t.putSharedPreference("scheme", "quanpin");
            t.putSharedPreference("touch_keyboard_layout", "twenty_six_key");
            t.putSharedPreference("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
            JSONObject voice = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), java.nio.charset.StandardCharsets.UTF_8))
                .getJSONObject("preferences").getJSONObject("voice_input");
            voice.put("asr_provider", "system").put("asr_model_path", "").put("polish_enabled", false);
            t.putSharedPreference("voice_input", voice);
            t.restartIme();
            t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
            t.tap(t.field("msime-test-plain"));
            text("");
            begin();
            t.stage = "actual cancel region and highlight";
            Rect cancel = bounds(t.described("取消本次语音"));
            Bitmap before = t.automation.takeScreenshot();
            move(cancel.centerX(), cancel.centerY());
            t.await(t.described("松开取消"));
            Bitmap pressed = t.automation.takeScreenshot();
            if (before == null || pressed == null) throw new AssertionError("Missing button pixels");
            boolean changed = false;
            for (int py = cancel.top + 4; py < cancel.bottom - 4; py += 3)
                for (int px = cancel.left + 4; px < cancel.right - 4; px += 3)
                    if (before.getPixel(px, py) != pressed.getPixel(px, py)) changed = true;
            before.recycle(); pressed.recycle();
            if (!changed) throw new AssertionError("Cancel did not highlight inside its actual bounds");
            move(cancel.left - 2, cancel.centerY());
            t.await(t.described("正在聆听：合成语音"));
            move(cancel.centerX(), cancel.centerY());
            end(MotionEvent.ACTION_UP);
            t.await(t.key("空格"));
            text("");

            begin();
            t.stage = "release on original space sends and restores keyboard";
            end(MotionEvent.ACTION_UP);
            text("合成语音");
            t.await(t.key("空格"));
            setText("");

            begin();
            t.stage = "horizontal right slide from the space releases on send";
            Rect hold = bounds(t.described("长按模式，松开发送"));
            if (!hold.contains((int) x, (int) y)) throw new AssertionError("Held space is not over 松开发送: " + hold);
            Rect send = bounds(t.described("发送本次语音"));
            if (y < send.top || y >= send.bottom) throw new AssertionError("Send is not in the held row: " + send);
            t.stage = "hold panel keys keep key and row spacing";
            // 操作区按钮与键区一样带键距、行距，左右相邻和上下相邻的键帽之间都要留出缝隙（#10966）。
            Rect lockKey = bounds(t.described("滑到此处松手锁定录音"));
            if (hold.left <= lockKey.right || send.left <= hold.right)
                throw new AssertionError("Held row keys touch: " + lockKey + " " + hold + " " + send);
            Rect cancelKey = bounds(t.described("取消本次语音"));
            Rect symbolKey = bounds(t.described("锁定语音中打开符号"));
            if (symbolKey.top <= cancelKey.bottom || lockKey.top <= symbolKey.bottom)
                throw new AssertionError("Hold panel rows touch: " + cancelKey + " " + symbolKey + " " + lockKey);
            t.stage = "horizontal right slide from the space releases on send";
            move(send.centerX(), y);
            end(MotionEvent.ACTION_UP);
            text("合成语音");
            t.await(t.key("空格"));
            setText("");

            begin();
            t.stage = "lock keeps recognition active after a horizontal left slide";
            Rect lock = bounds(t.described("滑到此处松手锁定录音"));
            if (y < lock.top || y >= lock.bottom) throw new AssertionError("Lock is not in the held row: " + lock);
            move(lock.centerX(), y);
            end(MotionEvent.ACTION_UP);
            t.await(t.described("录音已锁定，点击发送或取消结束").and(node -> !node.isEnabled()));
            t.await(t.described("正在聆听：合成语音"));
            SystemClock.sleep(1800);
            text("");
            t.stage = "original action row stays covered while locked";
            for (android.view.accessibility.AccessibilityWindowInfo window : t.automation.getWindows())
                if (t.find(window.getRoot(), t.described("键盘功能行")) != null)
                    throw new AssertionError("Original keyboard action row remains visible");
            captureKeyboard();
            assertHoldKeyGaps();
            t.stage = "locked punctuation tap and swipe";
            t.tap(t.described("锁定语音：点按！，上滑？"));
            swipe(t.described("锁定语音：点按，，上滑。"));
            text("！。");
            t.stage = "locked edit popup and repeated toggle";
            t.tap(t.described("锁定语音编辑操作"));
            Rect other = bounds(t.described("锁定语音编辑操作"));
            Rect space = bounds(t.described("锁定语音空格"));
            if (space.bottom > other.top + 2) throw new AssertionError("Edit popup not above its anchor");
            t.stage = "locked edit popup keys keep key gaps";
            Rect newline = bounds(t.described("锁定语音换行"));
            Rect delete = bounds(t.described("锁定语音删除"));
            gap(newline.right, space.left, "popup 换行 and 空格");
            gap(space.right, delete.left, "popup 空格 and 删除");
            t.stage = "locked popup inserts a space";
            t.tap(t.described("锁定语音空格"));
            text("！。 ");
            t.stage = "locked popup deletes the space";
            t.tap(t.described("锁定语音删除"));
            text("！。");
            t.stage = "locked popup inserts a newline";
            t.tap(t.described("锁定语音换行"));
            text("！。\n");
            t.tap(t.described("锁定语音编辑操作"));
            t.stage = "symbol panel returns to the same locked recording";
            t.tap(t.described("锁定语音中打开符号"));
            t.await(t.described("符号面板"));
            t.tap(t.described("关闭符号面板"));
            t.await(t.described("录音已锁定，点击发送或取消结束"));
            t.stage = "explicit send uses edited cursor";
            select(0);
            t.tap(t.described("发送本次语音"));
            text("合成语音！。\n");
            t.await(t.key("空格"));
            setText("");

            begin();
            lock = bounds(t.described("滑到此处松手锁定录音"));
            move(lock.centerX(), lock.centerY()); end(MotionEvent.ACTION_UP);
            t.stage = "service finishes before explicit send without auto commit";
            t.shellDiscard("am broadcast -a app.msime.android.test.FINISH_SPEECH -p app.msime.android.test");
            t.await(t.described("录音已结束，等待发送或取消"));
            text("");
            t.tap(t.described("取消本次语音"));
            t.await(t.key("空格"));
            text("");

            begin();
            lock = bounds(t.described("滑到此处松手锁定录音"));
            move(lock.centerX(), lock.centerY()); end(MotionEvent.ACTION_UP);
            t.stage = "service result waits for explicit send";
            t.shellDiscard("am broadcast -a app.msime.android.test.FINISH_SPEECH -p app.msime.android.test");
            t.await(t.described("录音已结束，等待发送或取消"));
            text("");
            t.tap(t.described("发送本次语音"));
            text("合成语音");
            t.await(t.key("空格"));
            setText("");

            begin();
            t.stage = "system cancel discards the held recognition";
            end(MotionEvent.ACTION_CANCEL);
            t.await(t.key("空格"));
            text("");

            begin();
            lock = bounds(t.described("滑到此处松手锁定录音"));
            move(lock.centerX(), lock.centerY()); end(MotionEvent.ACTION_UP);
            t.stage = "editor changes after send keep late result out of editor";
            t.tap(t.described("发送本次语音"));
            setText("合成编辑");
            // 编辑器可能重启输入连接；保留为结果或随会话取消都必须避免迟到直插。
            SystemClock.sleep(2500);
            text("合成编辑");
            t.await(t.described("语音结果面板").or(t.key("空格")));
        } finally {
            if (touching) end(MotionEvent.ACTION_CANCEL);
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else t.publish(preferences, original);
            t.restartIme();
        }
    }

    /** 锁定后的操作键像普通键区一样彼此隔开（#10966）：同行相邻键留键距、上下行留行距，不贴屏幕边缘；最下一行的 🔓 和「发送」与上一行的「符」「其他」同列。 */
    private void assertHoldKeyGaps() {
        t.stage = "locked hold keys keep key and row gaps";
        Rect cancel = bounds(t.described("取消本次语音"));
        Rect symbol = bounds(t.described("锁定语音中打开符号"));
        Rect exclaim = bounds(t.described("锁定语音：点按！，上滑？"));
        Rect comma = bounds(t.described("锁定语音：点按，，上滑。"));
        Rect other = bounds(t.described("锁定语音编辑操作"));
        Rect lock = bounds(t.described("滑到此处松手锁定录音"));
        Rect held = bounds(t.described("录音已锁定，点击发送或取消结束"));
        Rect send = bounds(t.described("发送本次语音"));
        gap(symbol.right, exclaim.left, "符 and ！");
        gap(comma.right, other.left, "， and 其他");
        gap(lock.right, held.left, "lock and locked space");
        gap(held.right, send.left, "locked space and send");
        gap(cancel.bottom, symbol.top, "cancel row and utility row");
        gap(symbol.bottom, lock.top, "utility row and control row");
        Rect screen = new Rect();
        for (android.view.accessibility.AccessibilityWindowInfo window : t.automation.getWindows())
            if (window.getType() == android.view.accessibility.AccessibilityWindowInfo.TYPE_INPUT_METHOD)
                window.getBoundsInScreen(screen);
        gap(screen.left, symbol.left, "keyboard edge and 符");
        gap(other.right, screen.right, "其他 and keyboard edge");
        if (Math.abs(lock.right - symbol.right) > 1 || Math.abs(send.left - other.left) > 1)
            throw new AssertionError("Control row columns drift from the utility row: "
                + lock + " " + symbol + " " + send + " " + other);
    }

    private static void gap(int before, int after, String what) {
        if (after <= before) throw new AssertionError("No gap between " + what + ": " + before + " >= " + after);
    }

    /** 仅保存合成输入法窗口的像素，编辑器不进入截图。 */
    private void captureKeyboard() throws Exception {
        for (android.view.accessibility.AccessibilityWindowInfo window : t.automation.getWindows()) {
            if (window.getType() != android.view.accessibility.AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect bounds = new Rect();
            window.getBoundsInScreen(bounds);
            Bitmap screen = t.automation.takeScreenshot();
            if (screen == null) throw new AssertionError("Missing keyboard pixels");
            Bitmap keyboard = Bitmap.createBitmap(screen, bounds.left, bounds.top, bounds.width(), bounds.height());
            try (java.io.FileOutputStream output = new java.io.FileOutputStream(new File(
                    t.getTargetContext().getExternalFilesDir(null), "voice-hold-panel.png"))) {
                keyboard.compress(Bitmap.CompressFormat.PNG, 100, output);
            } finally { keyboard.recycle(); screen.recycle(); }
            return;
        }
        throw new AssertionError("Missing input method window");
    }

    private void begin() throws Exception {
        t.stage = "real held space opens voice panel";
        Rect space = bounds(t.key("空格"));
        x = space.centerX(); y = space.centerY();
        downAt = SystemClock.uptimeMillis();
        touching = true;
        event(MotionEvent.ACTION_DOWN);
        t.await(t.described("发送本次语音"));
        t.await(t.described("正在聆听：合成语音"));
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        t.await(match).getBoundsInScreen(bounds);
        return bounds;
    }

    private void text(String value) {
        t.await(t.field("msime-test-plain").and(node -> value.isEmpty()
            ? node.isShowingHintText() || node.getText() == null || node.getText().length() == 0
            : DeviceSmoke.equalsText(value, node.getText())));
    }

    private void setText(String value) {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value);
        if (!t.await(t.field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Synthetic editor cannot set text");
        text(value);
    }

    private void select(int position) {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, position);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, position);
        if (!t.await(t.field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic editor cannot select");
        t.await(t.field("msime-test-plain").and(node -> node.getTextSelectionStart() == position));
        SystemClock.sleep(200);
    }

    private void swipe(Predicate<AccessibilityNodeInfo> match) {
        Rect key = bounds(match);
        downAt = SystemClock.uptimeMillis(); x = key.centerX(); y = key.centerY(); touching = true;
        event(MotionEvent.ACTION_DOWN);
        move(x, key.top - key.height());
        end(MotionEvent.ACTION_UP);
    }

    private void move(float nextX, float nextY) { x = nextX; y = nextY; event(MotionEvent.ACTION_MOVE); SystemClock.sleep(150); }
    private void end(int action) { event(action); touching = false; SystemClock.sleep(150); }
    private void event(int action) {
        MotionEvent event = MotionEvent.obtain(downAt, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try { if (!t.automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection rejected"); }
        finally { event.recycle(); }
    }
}
