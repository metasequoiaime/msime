package app.msime.android.test;

import android.os.Build;
import android.os.SystemClock;
import android.os.ParcelFileDescriptor;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 日语使用和输入框重建后，中英键仍返回进入外语前的中文入口。 */
public final class ChineseLanguageReturnDeviceSmoke extends DeviceSmoke {
    private boolean japaneseToggle;
    private final java.util.List<String> japaneseTimingFailures = new java.util.ArrayList<>();

    @Override public void onCreate(android.os.Bundle arguments) {
        japaneseToggle = arguments != null && "true".equals(arguments.getString("japanese-toggle"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (japaneseToggle) return "Japanese toggle preference, repeated kana and flick input";
        return "Chinese nine/fourteen-key return after English, Japanese typing and editor refocus";
    }

    @Override protected void runChecks() throws Exception {
        if (japaneseToggle) {
            runJapaneseToggleChecks();
            return;
        }
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        File resources = new File(options.getString("resources")).getCanonicalFile();
        if (!resources.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Resources escaped test sandbox");
        java.util.List<File> stagedResources = new java.util.ArrayList<>();
        File local = new File(new File(root, "bootstrap/state"), "android-settings.json");
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        String originalIme = shellText("settings get secure default_input_method").trim();
        android.content.SharedPreferences modes = getTargetContext()
            .getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        try {
            stage = "prepare bundled Japanese resources";
            // 专用 AVD 可能保留瘦包的资源目录；只临时补齐当前完整 APK 随包的日文文件。
            for (String name : new String[] {"msime-japanese.dat", "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt"}) {
                File target = new File(resources, name);
                if (target.exists()) continue;
                stagedResources.add(target);
                try (var input = getTargetContext().getAssets().open("dictionary/" + name)) {
                    Files.copy(input, target.toPath());
                }
            }
            JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
                .put("settings", new JSONObject()) : read(local);
            settings.getJSONObject("settings").put("platform.android.language_key_cycle", true);
            publish(local, bytes(settings));
            for (String[] layout : new String[][] {{"nine_key", "全拼 9 键"}, {"fourteen_key", "全拼 14 键"}}) {
                stage = "prepare " + layout[0];
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                    .put("touch_keyboard_layout", layout[0])
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                        .put("quanpin").put("nine_key").put("fourteen_key")
                        .put("japanese_nine_key").put("japanese")).put("selected", layout[0]));
                publish(preferences, bytes(new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", values)));
                modes.edit().clear().commit();
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                await(scheme(layout[1]));
                for (int refocus = 0; refocus < 3; refocus++) {
                    stage = layout[0] + " English transition " + refocus;
                    tap(language("切换到英文输入"));
                    await(key("英"));
                    stage = layout[0] + " Japanese transition " + refocus;
                    AccessibilityNodeInfo nextLanguage = await(node -> equalsText("app.msime.android", node.getPackageName())
                        && equalsText("英", node.getText()) && node.getContentDescription() != null
                        && node.getContentDescription().toString().contains("切换到"));
                    if (!nextLanguage.getContentDescription().toString().contains("日语 9 键")) {
                        boolean bundled = new File(options.getString("resources"), "msime-japanese.dat").isFile();
                        boolean cycle = read(local).getJSONObject("settings").getBoolean("platform.android.language_key_cycle");
                        String target = nextLanguage.getContentDescription().toString().contains("日语 26 键") ? "Japanese26" : "other";
                        throw new AssertionError("Japanese9 target unavailable: " + target + ", bundled=" + bundled + ", cycle=" + cycle);
                    }
                    tap(language("切换到日语 9 键"));
                    stage = layout[0] + " Japanese layout ready " + refocus;
                    await(scheme("日语 9 键"));
                    stage = layout[0] + " Japanese kana input " + refocus;
                    tap(key("あ"));
                    stage = layout[0] + " Japanese commit " + refocus;
                    tap(key("確定"));
                    // 不重启服务：真实的输入框切换会再次读取出厂副本，并异步恢复日语实时偏好。
                    stage = layout[0] + " Japanese editor refocus " + refocus;
                    tap(field(refocus % 2 == 0 ? "msime-test-cursor-at-end" : "msime-test-plain"));
                    await(scheme("日语 9 键"));
                    SystemClock.sleep(1000);
                    stage = layout[0] + " Chinese return target " + refocus;
                    AccessibilityNodeInfo chineseTarget = await(node -> equalsText("app.msime.android", node.getPackageName())
                        && equalsText("中", node.getText()) && node.isClickable()
                        && node.getContentDescription() != null && node.getContentDescription().toString().contains("切换到"));
                    if (!chineseTarget.getContentDescription().toString().contains(layout[1]))
                        throw new AssertionError(chineseTarget.getContentDescription().toString().contains("全拼 26 键")
                            ? "Chinese return changed to Quanpin26 after editor refocus" : "Chinese return lost the original entry");
                    tap(language("切换到" + layout[1]));
                    stage = layout[0] + " returned layout " + refocus;
                    await(scheme(layout[1]));
                    if (layout[0].equals("nine_key")) await(key("ABC"));
                    else await(described("按键 Q W"));
                    JSONObject saved = read(preferences).getJSONObject("preferences");
                    if (!layout[0].equals(saved.getJSONObject("touch_keyboard_schemes").getString("selected"))
                            || !layout[0].equals(saved.getString("touch_keyboard_layout")))
                        throw new AssertionError("Returned Chinese selection was not saved");
                }
                revision = read(preferences).getLong("revision");
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            for (File file : stagedResources) Files.deleteIfExists(file.toPath());
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("Input modes restore failed");
            restartIme();
            if (!originalIme.isEmpty() && !originalIme.equals("null")) shellDiscard("ime set " + originalIme);
        }
    }

    /** 同一个系统输入连接上验证关闭、开启和热更新后的连点，不靠假实现代替实际假名输入。 */
    private void runJapaneseToggleChecks() throws Exception {
        japaneseTimingFailures.clear();
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File resources = new File(options.getString("resources")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()) || !resources.toPath().startsWith(root.toPath()))
            throw new AssertionError("Japanese fixture escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        String originalIme = shellText("settings get secure default_input_method").trim();
        android.content.SharedPreferences modes = getTargetContext()
            .getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        java.util.List<File> stagedResources = new java.util.ArrayList<>();
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        String[][] animations = {{"window_animation_scale", ""}, {"transition_animation_scale", ""},
            {"animator_duration_scale", ""}};
        for (String[] animation : animations) {
            animation[1] = shellText("settings get global " + animation[0]).trim();
            if (!animation[1].equals("null")) animation[1] = Float.toString(Float.parseFloat(animation[1]));
        }
        try {
            // 触摸注入即使异步也会等待窗口事务，先去掉系统动画，避免把动画时间算成用户的连点间隔。
            for (String[] animation : animations) shellDiscard("settings put global " + animation[0] + " 0");
            stage = "prepare Japanese toggle fixture";
            for (String name : new String[] {"msime-japanese.dat", "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt"}) {
                File target = new File(resources, name);
                if (target.exists()) continue;
                stagedResources.add(target);
                try (var input = getTargetContext().getAssets().open("dictionary/" + name)) {
                    Files.copy(input, target.toPath());
                }
            }
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "japanese").put("touch_keyboard_layout", "nine_key")
                .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                .put("touch_japanese_toggle_input", false)
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                    .put("quanpin").put("japanese_nine_key")).put("selected", "japanese_nine_key"));
            publish(preferences, bytes(new JSONObject().put("format_version", 1)
                .put("revision", ++revision).put("preferences", values)));
            if (!modes.edit().clear().commit()) throw new AssertionError("Japanese fixture input modes reset failed");
            restartIme();
            stage = "launch Japanese toggle editor";
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            stage = "Japanese toggle keyboard ready";
            awaitJapaneseReady();
            stage = "disabled toggle accessibility matches preference";
            await(describedPrefix("轻点输入さ；"));
            // 先确认日语会话真正接受输入，再测稳态连点；首个按键可能还承担词典和引擎的首次准备。
            stage = "prime Japanese toggle input";
            tap(japaneseKana("あ"));
            await(field("msime-test-plain").and(node -> equalsText("あ", node.getText())));
            stage = "clear Japanese toggle primer";
            tap(key("⌫"));
            // 空编辑器的无障碍文字可能是提示语，按提示状态判断实际空值。
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            stage = "disabled toggle held preview matches repeated center kana";
            tap(japaneseKana("さ"));
            await(field("msime-test-plain").and(node -> equalsText("さ", node.getText())));
            captureJapaneseCenterPreview();
            await(field("msime-test-plain").and(node -> equalsText("ささ", node.getText())));
            tap(key("⌫"));
            tap(key("⌫"));
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            for (String kana : new String[] {"さ", "や"}) {
                stage = "disabled toggle repeats kana " + kana;
                repeatKana(kana);
                await(field("msime-test-plain").and(node -> equalsText(kana + kana, node.getText())));
                tap(key("⌫"));
                tap(key("⌫"));
                await(field("msime-test-plain").and(node -> node.isShowingHintText()
                    || node.getText() == null || node.getText().length() == 0));
            }
            stage = "disabled toggle retains flick";
            android.graphics.Rect bounds = new android.graphics.Rect();
            await(japaneseKana("さ")).getBoundsInScreen(bounds);
            long now = SystemClock.uptimeMillis();
            for (int action : new int[] {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_MOVE,
                    android.view.MotionEvent.ACTION_UP}) {
                float x = action == android.view.MotionEvent.ACTION_DOWN ? bounds.centerX() : bounds.left - 15;
                var event = android.view.MotionEvent.obtain(now, SystemClock.uptimeMillis(), action, x, bounds.centerY(), 0);
                try {
                    event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
                    if (!automation.injectInputEvent(event, true)) throw new AssertionError("Japanese flick injection failed");
                } finally { event.recycle(); }
            }
            await(field("msime-test-plain").and(node -> equalsText("し", node.getText())));
            tap(key("⌫"));
            stage = "toggle preference enabled live";
            values.put("touch_japanese_toggle_input", true);
            publish(preferences, bytes(new JSONObject().put("format_version", 1)
                .put("revision", ++revision).put("preferences", values)));
            await(describedPrefix("轻点输入さ，连续轻点依次切换"));
            for (String[] pair : new String[][] {{"さ", "し"}, {"や", "ゆ"}}) {
                if (repeatKana(pair[0]))
                    await(field("msime-test-plain").and(node -> equalsText(pair[1], node.getText())));
                // 计时前提失败时保留失败记录，清空两种可能的输出后继续独立验收；最后仍以失败结束。
                tap(key("⌫"));
                tap(key("⌫"));
                await(field("msime-test-plain").and(node -> node.isShowingHintText()
                    || node.getText() == null || node.getText().length() == 0));
            }
            android.os.Bundle text = new android.os.Bundle();
            text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "ab");
            if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                throw new AssertionError("Japanese caret fixture text failed");
            android.os.Bundle selection = new android.os.Bundle();
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 1);
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 1);
            if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection))
                throw new AssertionError("Japanese caret fixture selection failed");
            tap(japaneseKana("、"));
            await(field("msime-test-plain").and(node -> equalsText("a、b", node.getText())
                && node.getTextSelectionStart() == 2));
            stage = "toggle preference disabled clears old sequence";
            values.put("touch_japanese_toggle_input", false);
            publish(preferences, bytes(new JSONObject().put("format_version", 1)
                .put("revision", ++revision).put("preferences", values)));
            await(describedPrefix("轻点输入さ；"));
            stage = "toggle preference enabled after clearing old sequence";
            values.put("touch_japanese_toggle_input", true);
            publish(preferences, bytes(new JSONObject().put("format_version", 1)
                .put("revision", ++revision).put("preferences", values)));
            await(describedPrefix("轻点输入さ，连续轻点依次切换"));
            // 右箭头不受一秒时间窗限制：旧连点残留时只结束连点，清理后会真正越过尾部字符。
            stage = "right arrow after Japanese toggle reset";
            tap(describedPrefix("按键 结束连点切换，开始下一个假名"));
            stage = "right arrow moves caret after Japanese toggle reset";
            await(field("msime-test-plain").and(node -> equalsText("a、b", node.getText())
                && node.getTextSelectionStart() == 3));
            stage = "Japanese setting saves from native input page";
            android.os.Bundle args = new android.os.Bundle();
            args.putString("section", "语言");
            android.content.Intent intent = new android.content.Intent()
                .setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
                .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK
                    | android.content.Intent.FLAG_ACTIVITY_CLEAR_TOP | android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra("msime_open_page", "TYPING").putExtra("msime_page_args", args);
            getTargetContext().startActivity(intent);
            Predicate<AccessibilityNodeInfo> setting = node -> equalsText("app.msime.android", node.getPackageName())
                && node.isCheckable() && find(node, child -> equalsText("日语九键连点切换", child.getText())) != null;
            await(setting.and(AccessibilityNodeInfo::isChecked));
            tap(setting);
            long deadline = SystemClock.uptimeMillis() + 10000;
            while (read(preferences).getJSONObject("preferences").optBoolean("touch_japanese_toggle_input", true)
                    && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(100);
            if (read(preferences).getJSONObject("preferences").optBoolean("touch_japanese_toggle_input", true))
                throw new AssertionError("Japanese toggle setting did not persist false");
            getTargetContext().startActivity(intent);
            await(setting.and(node -> !node.isChecked()));
            if (!japaneseTimingFailures.isEmpty()) {
                stage = "Japanese repeated tap timing premises";
                throw new AssertionError(String.join("; ", japaneseTimingFailures));
            }
        } finally {
            try {
                shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
                restore(preferences, original);
                for (File file : stagedResources) Files.deleteIfExists(file.toPath());
                android.content.SharedPreferences.Editor restored = modes.edit().clear();
                for (var entry : originalModes.entrySet()) restored.putBoolean(entry.getKey(), (Boolean) entry.getValue());
                if (!restored.commit()) throw new AssertionError("Japanese fixture input modes restore failed");
                restartIme();
                if (!originalIme.isEmpty() && !originalIme.equals("null")) shellDiscard("ime set " + originalIme);
            } finally {
                // 原本没设过时不能只删掉这一项：窗口管理器读不到值就沿用它内存里的上一个值，也就是这里设的 0，之后的套件全在没有动画的系统上跑（`settings get` 却报 null）。2026-10-25 API 35 上剪贴板导出因此等不到「这台设备没有可用的系统认证」提示。先写回默认的 1 让系统真正恢复，再删掉，回到没设过的状态。
                for (String[] animation : animations) {
                    if (animation[1].equals("null")) {
                        shellDiscard("settings put global " + animation[0] + " 1");
                        shellDiscard("settings delete global " + animation[0]);
                    } else {
                        shellDiscard("settings put global " + animation[0] + " " + animation[1]);
                    }
                }
            }
        }
    }

    /** 只保存合成测试的键区，供核对关闭连点后第二次按住的中心假名。 */
    private void captureJapaneseCenterPreview() throws Exception {
        android.graphics.Rect bounds = new android.graphics.Rect();
        await(japaneseKana("さ")).getBoundsInScreen(bounds);
        long down = SystemClock.uptimeMillis();
        for (int action : new int[] {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_UP}) {
            var event = android.view.MotionEvent.obtain(down, SystemClock.uptimeMillis(), action,
                bounds.centerX(), bounds.centerY(), 0);
            try {
                event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(event, true)) throw new AssertionError("Japanese preview touch failed");
            } finally { event.recycle(); }
            if (action != android.view.MotionEvent.ACTION_DOWN) continue;
            try {
                android.graphics.Bitmap screen = awaitJapanesePreviewFrame(bounds);
                try {
                    int left = Math.max(0, bounds.left - bounds.width() / 2);
                    int top = Math.max(0, bounds.top - bounds.height() / 2);
                    int right = Math.min(screen.getWidth(), bounds.right + bounds.width() / 2);
                    int bottom = Math.min(screen.getHeight(), bounds.bottom + bounds.height() / 2);
                    android.graphics.Bitmap crop = android.graphics.Bitmap.createBitmap(screen,
                        left, top, right - left, bottom - top);
                    try (var output = new java.io.FileOutputStream(new File(getTargetContext().getFilesDir(),
                            "japanese-toggle-disabled-held.png"))) {
                        if (!crop.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output))
                            throw new AssertionError("Japanese preview screenshot was not saved");
                    } finally { crop.recycle(); }
                } finally { screen.recycle(); }
            } catch (Exception | AssertionError failure) {
                var cancel = android.view.MotionEvent.obtain(down, SystemClock.uptimeMillis(),
                    android.view.MotionEvent.ACTION_CANCEL, bounds.centerX(), bounds.centerY(), 0);
                try { automation.injectInputEvent(cancel, true); }
                finally { cancel.recycle(); }
                throw failure;
            }
        }
    }

    /** 提示不发无障碍事件，界面空闲不代表已绘制；等中心强调格与角落底板形成色差后再截图。 */
    private android.graphics.Bitmap awaitJapanesePreviewFrame(android.graphics.Rect bounds) {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            android.graphics.Bitmap screen = automation.takeScreenshot();
            if (screen == null) {
                SystemClock.sleep(50);
                continue;
            }
            int center = dominantColor(screen, bounds, 0.36f, 0.36f, 0.64f, 0.64f);
            int corner = dominantColor(screen, bounds, 0.12f, 0.12f, 0.22f, 0.22f);
            int contrast = Math.abs(android.graphics.Color.red(center) - android.graphics.Color.red(corner))
                + Math.abs(android.graphics.Color.green(center) - android.graphics.Color.green(corner))
                + Math.abs(android.graphics.Color.blue(center) - android.graphics.Color.blue(corner));
            if (contrast >= 64) return screen;
            screen.recycle();
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Japanese preview center was not painted");
    }

    /** 多点取众数以避开假名字形；普通整键按下变色不会被当成中心格已经出现。 */
    private static int dominantColor(android.graphics.Bitmap screen, android.graphics.Rect bounds,
            float left, float top, float right, float bottom) {
        java.util.Map<Integer, Integer> colors = new java.util.HashMap<>();
        int dominant = 0;
        int most = 0;
        for (int y = 0; y < 7; y++) {
            for (int x = 0; x < 7; x++) {
                int color = screen.getPixel(bounds.left + Math.round(bounds.width() * (left + (right - left) * x / 6)),
                    bounds.top + Math.round(bounds.height() * (top + (bottom - top) * y / 6)));
                int count = colors.getOrDefault(color, 0) + 1;
                colors.put(color, count);
                if (count > most) {
                    dominant = color;
                    most = count;
                }
            }
        }
        return dominant;
    }

    /** 冷启动在共享构建主机上曾超过通用的 15 秒期限；只等会话可用，不改变后面的连点时间窗。 */
    private void awaitJapaneseReady() {
        long deadline = SystemClock.uptimeMillis() + 120000;
        String readiness = "scheme absent";
        do {
            for (var window : automation.getWindows()) {
                AccessibilityNodeInfo button = find(window.getRoot(), scheme("日语 9 键"));
                if (button == null) continue;
                if (button.isEnabled()) return;
                CharSequence state = Build.VERSION.SDK_INT >= 30 ? button.getStateDescription() : null;
                // 只记录产品固定状态，不读取输入框或偏好内容。
                readiness = equalsText("输入会话未就绪", state) ? "session unavailable"
                    : equalsText("设置加载中", state) ? "preferences loading" : "other";
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Japanese scheme readiness: " + readiness);
    }

    /** 候选栏也可能出现同名假名，按键必须通过专属读屏说明定位。 */
    private Predicate<AccessibilityNodeInfo> japaneseKana(String kana) {
        return describedPrefix("轻点输入" + kana);
    }

    private boolean repeatKana(String kana) throws Exception {
        AccessibilityNodeInfo node = await(japaneseKana(kana));
        long[] clicks = {-1, -1};
        int[] count = {0};
        // 无障碍动作直接点击真实按键，以 IME 发出的两次事件计时，避开触摸注入的窗口事务等待。
        automation.executeAndWaitForEvent(() -> {
            if (!node.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                    || !node.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("Japanese repeated tap action failed");
        }, event -> {
            if (event.getEventType() != android.view.accessibility.AccessibilityEvent.TYPE_VIEW_CLICKED
                    || !equalsText("app.msime.android", event.getPackageName())
                    || event.getText().stream().noneMatch(text -> equalsText(kana, text))) return false;
            clicks[count[0]++] = event.getEventTime();
            return count[0] == 2;
        }, 10000);
        long elapsed = clicks[1] - clicks[0];
        if (elapsed < 0 || elapsed >= 1000) {
            japaneseTimingFailures.add("Japanese repeated tap fixture exceeded toggle window: " + kana + " " + elapsed + "ms");
            return false;
        }
        return true;
    }

    private Predicate<AccessibilityNodeInfo> scheme(String title) {
        return describedPrefix("输入方案：" + title);
    }

    private Predicate<AccessibilityNodeInfo> language(String description) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isClickable()
            && (equalsText(description, node.getContentDescription())
                || equalsText("按键 " + description, node.getContentDescription()));
    }

    // API 28 的节点缓存不会自动失效，必须刷新后再判断重建后的键盘。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private String shellText(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            for (int count; (count = input.read(buffer)) != -1;) output.write(buffer, 0, count);
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static byte[] bytes(JSONObject value) { return value.toString().getBytes(StandardCharsets.UTF_8); }

    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else publish(file, original);
    }
}
