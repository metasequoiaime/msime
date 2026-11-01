package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.os.SystemClock;
import android.text.InputType;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import org.json.JSONObject;

/** 安装后的原生九键预览与复用视图反馈设置；不依赖模拟器是否有真实马达。 */
final class KeyboardFeedbackDeviceChecks {
    private final DeviceSmoke test;
    private Object service;
    private Class<?> type;
    private Object letters;
    private long down;
    private final int[] clicks = {0};
    private File feedback;
    private byte[] original;

    KeyboardFeedbackDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run(String mode) throws Exception {
        Activity activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        feedback = new File(test.getTargetContext().getFilesDir(), "bootstrap/state/keyboard-feedback.json");
        original = feedback.isFile() ? Files.readAllBytes(feedback.toPath()) : null;
        try {
            onMain(() -> {
                type = test.getTargetContext().getClassLoader().loadClass("app.msime.android.MSIMEInputService");
                service = type.getConstructor().newInstance();
                method(ContextWrapper.class, "attachBaseContext", Context.class).invoke(service, test.getTargetContext());
                type.getMethod("onCreate").invoke(service);
                View root = (View) type.getMethod("onCreateInputView").invoke(service);
                activity.setContentView(root);
                letters = field(type, "imeLetterRows").get(service);
                field(type, "soundEnabled").setBoolean(service, false);
                field(type, "hapticsEnabled").setBoolean(service, false);
                field(type, "editorInputType").setInt(service, InputType.TYPE_CLASS_TEXT);
            });
            test.waitForIdleSync();
            if ("sound-only".equals(mode)) { checkSound(); return; }
            if (!"preview-only".equals(mode)) checkRefresh();
            onMain(() -> {
                test.stage = "nine-key preview opens on touch down";
                field(type, "soundEnabled").setBoolean(service, false);
                field(type, "hapticsEnabled").setBoolean(service, false);
                setLocal(true);
                Object rows = field(type, "imeLayoutRows").get(service);
                ViewGroup keys = (ViewGroup) field(type, "keyRows").get(service);
                keys.removeAllViews();
                method(rows.getClass(), "rebuildNineKeyRows").invoke(rows);
            });
            test.waitForIdleSync();
            onMain(() -> {
                ViewGroup keys = (ViewGroup) field(type, "keyRows").get(service);
                Button abc = find(keys, "ABC"), def = find(keys, "DEF");
                if (abc == null || def == null) throw new AssertionError("Synthetic nine-key grid missing");
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                expectPreview("ABC", true);
                test.stage = "nine-key preview ownership follows the latest finger";
                touch(def, MotionEvent.ACTION_DOWN, 0);
                touch(abc, MotionEvent.ACTION_CANCEL, 0);
                expectPreview("DEF", true);
                touch(def, MotionEvent.ACTION_CANCEL, 0);
                expectPreview("DEF", false);
                test.stage = "nine-key release hides preview and clicks exactly once";
                abc.setOnClickListener(ignored -> clicks[0]++);
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                touch(abc, MotionEvent.ACTION_UP, 0);
                expectPreview("ABC", false);
                test.stage = "nine-key numeric swipe updates and cancellation hides preview";
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                touch(abc, MotionEvent.ACTION_MOVE, -100);
                expectPreview("2", true);
                touch(abc, MotionEvent.ACTION_CANCEL, -100);
                expectPreview("2", false);
                if (!"ABC".contentEquals(abc.getText())) throw new AssertionError("Cancelled swipe left numeric face");
            });
            checkDragSelect();
            onMain(() -> {
                Button abc = find((ViewGroup) field(type, "keyRows").get(service), "ABC");
                test.stage = "long press closes preview before showing choices";
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                abc.performLongClick();
                expectPreview("ABC", false);
                if (field(type, "nineKeyHoldPopup").get(service) == null)
                    throw new AssertionError("Nine-key long press lost choices");
                method(field(type, "imeLayoutRows").get(service).getClass(), "dismissNineKeyHoldOptions")
                    .invoke(field(type, "imeLayoutRows").get(service));
                touch(abc, MotionEvent.ACTION_CANCEL, 0);
                test.stage = "disabled and password previews stay hidden";
                setLocal(false);
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                expectPreview("ABC", false);
                touch(abc, MotionEvent.ACTION_CANCEL, 0);
                setLocal(true);
                field(type, "editorInputType").setInt(service, InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
                touch(abc, MotionEvent.ACTION_DOWN, 0);
                expectPreview("ABC", false);
                touch(abc, MotionEvent.ACTION_CANCEL, 0);
            });
            test.waitForIdleSync();
            onMain(() -> {
                if (clicks[0] != 1) throw new AssertionError("Nine-key tap was swallowed or duplicated");
            });
        } finally {
            onMain(() -> {
                if (service != null) {
                    type.getMethod("onDestroy").invoke(service);
                    for (Field member : type.getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        member.setAccessible(true);
                        ((ExecutorService) member.get(service)).shutdownNow();
                    }
                }
                activity.finish();
            });
            if (original == null) Files.deleteIfExists(feedback.toPath()); else test.publish(feedback, original);
        }
    }

    /**
     * 关掉系统触摸提示音并卸载系统音效（系统设置页关掉开关时就是这样做的），这时系统按键音一声不响；默认的「清脆键盘」包要由本进程自己的 SoundPool 播出来。直接观察 Android 音频服务的播放记录，不依靠听模拟器声音。新版系统只在播放器状态变化时记一笔，同一个 SoundPool 第二次播放不再留下记录，所以这里只断言第一次。
     */
    /**
     * 九键反方向滑出、长按弹出的数字与字母选项跟着同一次按压拖选（#10443）：滑进选项松手选中一次；没滑进过就松手，弹窗留着等点选，下一次按下这个键时关掉；滑进又滑出再松手关闭且不输入；长按后拖选松手选中，并由补发的 CANCEL 收起按键的按下态；系统 CANCEL 关闭弹窗且不输入。选项的点击换成计数，不依赖输入连接；计数之后照原样关闭弹窗。
     */
    private void checkDragSelect() throws Exception {
        int[] picked = {0};
        Object[] rows = new Object[1];
        Button[] abc = new Button[1];
        onMain(() -> {
            rows[0] = field(type, "imeLayoutRows").get(service);
            abc[0] = find((ViewGroup) field(type, "keyRows").get(service), "ABC");
            test.stage = "nine-key letter swipe opens drag-tracked options";
            swipeLetters(abc[0], rows[0]);
        });
        test.waitForIdleSync();
        onMain(() -> {
            test.stage = "nine-key swipe dragged into an option commits it on release";
            float[] at = pickable(rows[0], picked);
            touchRaw(abc[0], MotionEvent.ACTION_MOVE, at[0], at[1]);
            touchRaw(abc[0], MotionEvent.ACTION_UP, at[0], at[1]);
            if (picked[0] != 1) throw new AssertionError("Dragged nine-key option was not committed once");
            if (field(type, "nineKeyHoldPopup").get(service) != null) throw new AssertionError("Committed option left popup open");
            if (abc[0].isPressed()) throw new AssertionError("Nine-key swipe left the key pressed");
            if (!"ABC".contentEquals(abc[0].getText())) throw new AssertionError("Letter swipe changed the key face");
            test.stage = "nine-key swipe released without entering an option keeps the popup for tapping";
            swipeLetters(abc[0], rows[0]);
            touch(abc[0], MotionEvent.ACTION_UP, 100);
            if (field(type, "nineKeyHoldPopup").get(service) == null) throw new AssertionError("Released swipe closed the options");
            if (field(rows[0].getClass(), "letterOptionsEntered").getBoolean(rows[0]))
                throw new AssertionError("Released swipe counted as entering an option");
            touch(abc[0], MotionEvent.ACTION_DOWN, 0);
            if (tracks(rows[0], abc[0]) || field(type, "nineKeyHoldPopup").get(service) != null)
                throw new AssertionError("New press on the key kept the stale options");
            touch(abc[0], MotionEvent.ACTION_CANCEL, 0);
            test.stage = "nine-key swipe entering then leaving the options closes them without input";
            swipeLetters(abc[0], rows[0]);
        });
        test.waitForIdleSync();
        onMain(() -> {
            float[] at = pickable(rows[0], picked);
            touchRaw(abc[0], MotionEvent.ACTION_MOVE, at[0], at[1]);
            touchRaw(abc[0], MotionEvent.ACTION_MOVE, at[0], at[1] - 500);
            touchRaw(abc[0], MotionEvent.ACTION_UP, at[0], at[1] - 500);
            if (field(type, "nineKeyHoldPopup").get(service) != null) throw new AssertionError("Left options stayed open");
            if (picked[0] != 1) throw new AssertionError("Leaving the options still committed one");
            test.stage = "nine-key long press options follow the same press";
            touch(abc[0], MotionEvent.ACTION_DOWN, 0);
        });
        // 走框架真实的长按计时，让按钮自己记下「已长按」和按下态，补发的 CANCEL 才有东西可收。
        long until = SystemClock.uptimeMillis() + android.view.ViewConfiguration.getLongPressTimeout() + 5000;
        boolean[] opened = {false};
        while (!opened[0] && SystemClock.uptimeMillis() < until) {
            SystemClock.sleep(100);
            onMain(() -> opened[0] = tracks(rows[0], abc[0]));
        }
        if (!opened[0]) throw new AssertionError("Nine-key long press did not open drag-tracked options");
        test.waitForIdleSync();
        onMain(() -> {
            test.stage = "nine-key long press dragged into an option commits it and releases the key";
            float[] at = pickable(rows[0], picked);
            touchRaw(abc[0], MotionEvent.ACTION_MOVE, at[0], at[1]);
            touchRaw(abc[0], MotionEvent.ACTION_UP, at[0], at[1]);
            if (picked[0] != 2) throw new AssertionError("Long-press drag did not commit the option once");
            if (abc[0].isPressed()) throw new AssertionError("Synthesized CANCEL did not release the long-pressed key");
            test.stage = "system cancel closes swipe options without input";
            swipeLetters(abc[0], rows[0]);
            touch(abc[0], MotionEvent.ACTION_CANCEL, 100);
            if (field(type, "nineKeyHoldPopup").get(service) != null) throw new AssertionError("Cancelled swipe left options open");
            if (picked[0] != 2) throw new AssertionError("Cancelled swipe committed an option");
            if (!"ABC".contentEquals(abc[0].getText())) throw new AssertionError("Cancelled letter swipe changed the key face");
        });
    }

    /** setLocal 把数字设在上滑，下滑越过阈值就弹出字母选项，并且这个弹窗跟着这次按压拖选。 */
    private void swipeLetters(Button key, Object rows) throws Exception {
        touch(key, MotionEvent.ACTION_DOWN, 0);
        touch(key, MotionEvent.ACTION_MOVE, 100);
        if (field(type, "nineKeyHoldPopup").get(service) == null || !tracks(rows, key))
            throw new AssertionError("Nine-key letter swipe did not open drag-tracked options");
    }

    private static boolean tracks(Object rows, Button key) throws Exception {
        return (boolean) method(rows.getClass(), "tracksLetterOptions", Button.class).invoke(rows, key);
    }

    /** 取字母 a 那个选项（第 0 个是数字 2），点击换成计数后照原样关闭弹窗；返回它在屏幕上的中心点。 */
    private float[] pickable(Object rows, int[] picked) throws Exception {
        @SuppressWarnings("unchecked")
        java.util.List<Button> options = (java.util.List<Button>) field(rows.getClass(), "letterOptionButtons").get(rows);
        if (options.size() < 2 || !"a".contentEquals(options.get(1).getText()))
            throw new AssertionError("Nine-key options are not 2 followed by letters");
        Button option = options.get(1);
        if (!option.isShown() || option.getWidth() == 0) throw new AssertionError("Nine-key options were not laid out");
        Method dismiss = method(rows.getClass(), "dismissNineKeyHoldOptions");
        option.setOnClickListener(ignored -> {
            picked[0]++;
            try { dismiss.invoke(rows); } catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
        });
        int[] location = new int[2];
        option.getLocationOnScreen(location);
        return new float[] {location[0] + option.getWidth() / 2f, location[1] + option.getHeight() / 2f};
    }

    private void checkSound() throws Exception {
        String beforeSetting = shell("settings get system sound_effects_enabled").trim();
        android.media.AudioManager audio = (android.media.AudioManager)
            test.getTargetContext().getSystemService(Context.AUDIO_SERVICE);
        try {
            test.shellDiscard("settings put system sound_effects_enabled 0");
            onMain(audio::unloadSoundEffects);
            test.stage = "default key sound pack loads";
            playSound(true);
            long until = SystemClock.uptimeMillis() + 5000;
            while (!defaultPackLoaded() && SystemClock.uptimeMillis() < until) SystemClock.sleep(150);
            if (!defaultPackLoaded()) throw new AssertionError("Default key sound pack did not load");
            // SoundPool 解码样本是异步的，留出时间再播。
            SystemClock.sleep(800);
            test.stage = "default key sound plays from the app when system effects are unloaded";
            expectOwnPlayback(true);
            SystemClock.sleep(500);
            String before = lastPlayback();
            test.stage = "disabled app key sound produces no playback";
            playSound(false);
            SystemClock.sleep(700);
            if (!before.equals(lastPlayback())) throw new AssertionError("Disabled key sound still played");
        } finally {
            test.shellDiscard("null".equals(beforeSetting) ? "settings delete system sound_effects_enabled"
                : "settings put system sound_effects_enabled " + ("1".equals(beforeSetting) ? "1" : "0"));
            if (!"0".equals(beforeSetting)) onMain(audio::loadSoundEffects);
        }
    }

    private void expectOwnPlayback(boolean enabled) throws Exception {
        int before = ownStarts();
        playSound(enabled);
        long until = SystemClock.uptimeMillis() + 3000;
        while (ownStarts() == before && SystemClock.uptimeMillis() < until) SystemClock.sleep(150);
        if (ownStarts() == before) throw new AssertionError("Key sound produced no playback from the app's SoundPool");
    }

    private boolean defaultPackLoaded() throws Exception {
        boolean[] loaded = {false};
        onMain(() -> {
            Object player = field(type, "imeKeyFeedback").get(service);
            Object sounds = field(player.getClass(), "sounds").get(player);
            loaded[0] = sounds != null && "default".equals(method(sounds.getClass(), "id").invoke(sounds));
        });
        return loaded[0];
    }

    private void playSound(boolean enabled) throws Exception {
        onMain(() -> {
            field(type, "soundEnabled").setBoolean(service, enabled);
            field(type, "hapticsEnabled").setBoolean(service, false);
            field(type, "preferencesSnapshot").set(service, new JSONObject().put("preferences", new JSONObject()));
            Object player = field(type, "imeKeyFeedback").get(service);
            method(player.getClass(), "playFeedback", View.class).invoke(player, new Button((Context) service));
        });
    }

    private String lastPlayback() throws Exception {
        String last = "";
        for (String line : shell("dumpsys audio").split("\n"))
            if (line.contains("player piid:") && started(line)) last = line.trim();
        return last;
    }

    /** API 28 把开始播放记成 `state:started`，新版记成 `event:started`。 */
    private static boolean started(String line) {
        return line.contains("state:started") || line.contains("event:started");
    }

    /** 本进程的 SoundPool 播放器在音频服务里记下的开始播放次数；系统按键音的 SoundPool 属于 system_server，不计在内。播放器列表各版本格式不同，按同一行里的 `piid`、SoundPool 类型和本进程的 uid/pid 认。 */
    private int ownStarts() throws Exception {
        String[] lines = shell("dumpsys audio").split("\n");
        String owner = "/pid:" + android.os.Process.myUid() + "/" + android.os.Process.myPid() + " ";
        java.util.Set<String> players = new java.util.HashSet<>();
        for (String line : lines) {
            if (!line.contains("android.media.SoundPool") || !line.contains(owner)) continue;
            String piid = idAfter(line, "piid:");
            if (piid == null) piid = idAfter(line, "ID:");
            if (piid != null) players.add(piid);
        }
        int starts = 0;
        for (String line : lines)
            if (line.contains("player piid:") && started(line) && players.contains(idAfter(line, "player piid:"))) starts++;
        return starts;
    }

    private static String idAfter(String line, String label) {
        int at = line.indexOf(label);
        if (at < 0) return null;
        int from = at + label.length(), to = from;
        while (to < line.length() && Character.isDigit(line.charAt(to))) to++;
        return to > from ? line.substring(from, to) : null;
    }

    private String shell(String command) throws Exception {
        try (var input = new android.os.ParcelFileDescriptor.AutoCloseInputStream(test.automation.executeShellCommand(command))) {
            java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[4096];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }

    private void checkRefresh() throws Exception {
        for (boolean enabled : new boolean[] {true, false}) {
            test.publish(feedback, new JSONObject().put("soundEnabled", enabled).put("hapticsEnabled", enabled)
                .put("hapticStrength", enabled ? "strong" : "light").toString().getBytes(StandardCharsets.UTF_8));
            onMain(() -> {
                test.stage = "reused input view immediately reads feedback before delayed refresh";
                type.getMethod("onStartInputView", EditorInfo.class, boolean.class).invoke(service, new EditorInfo(), true);
                if (field(type, "soundEnabled").getBoolean(service) != enabled
                        || field(type, "hapticsEnabled").getBoolean(service) != enabled)
                    throw new AssertionError("Reused input view kept stale feedback switches");
                Object strength = field(type, "hapticStrength").get(service);
                String id = (String) strength.getClass().getMethod("id").invoke(strength);
                if (!id.equals(enabled ? "strong" : "light")) throw new AssertionError("Reused view kept stale strength");
                method(type, "cancelInputViewRefresh").invoke(service);
            });
        }
    }

    private void setLocal(boolean popup) throws Exception {
        Class<?> snapshot = test.getTargetContext().getClassLoader().loadClass("app.msime.android.AndroidLocalSettings$Snapshot");
        var constructor = snapshot.getDeclaredConstructor(Map.class);
        constructor.setAccessible(true);
        field(type, "localSettings").set(service, constructor.newInstance(Map.of("platform.android.key_popup", popup)));
        // 九键滑动输入数字在共享偏好 touch_nine_key_swipe_digits 里（#10229）；这个合成服务没有会话、不读偏好文件，直接给它读到的值，与 applyTouchGeometry 读到 "up" 时相同。
        field(type, "nineKeySwipeDigits").set(service, "up");
    }

    private void expectPreview(String label, boolean shown) throws Exception {
        View preview = (View) field(letters.getClass(), "keyPreview").get(letters);
        if (shown) {
            if (preview == null || preview.getVisibility() != View.VISIBLE)
                throw new AssertionError("Nine-key preview did not appear");
            if (!label.equals(preview.getClass().getMethod("label").invoke(preview)))
                throw new AssertionError("Nine-key preview has stale label");
        } else if (preview != null && preview.getVisibility() == View.VISIBLE)
            throw new AssertionError("Nine-key preview stayed visible");
    }

    private void touch(Button key, int action, float deltaDp) {
        if (action == MotionEvent.ACTION_DOWN) down = SystemClock.uptimeMillis();
        float delta = deltaDp * test.getTargetContext().getResources().getDisplayMetrics().density;
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action,
            key.getWidth() / 2f, key.getHeight() / 2f + delta, 0);
        try { key.dispatchTouchEvent(event); } finally { event.recycle(); }
    }

    /** 直接派发给按钮的事件没有窗口偏移，getRawX/getRawY 就是这里给的屏幕坐标，与 moveLetterOptions 用 getLocationOnScreen 做的命中判断同一坐标系。 */
    private void touchRaw(Button key, int action, float rawX, float rawY) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, rawX, rawY, 0);
        try { key.dispatchTouchEvent(event); } finally { event.recycle(); }
    }

    private static Button find(View view, String text) {
        if (view instanceof Button button && text.contentEquals(button.getText())) return button;
        if (view instanceof ViewGroup group) for (int i = 0; i < group.getChildCount(); i++) {
            Button result = find(group.getChildAt(i), text); if (result != null) return result;
        }
        return null;
    }

    private void onMain(Checked action) throws Exception {
        Throwable[] failure = {null};
        test.runOnMainSync(() -> { try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; } });
        if (failure[0] instanceof AssertionError error) throw error;
        if (failure[0] instanceof Exception error) throw error;
    }
    @FunctionalInterface private interface Checked { void run() throws Exception; }
    private static Field field(Class<?> type, String name) throws Exception {
        Field value = type.getDeclaredField(name); value.setAccessible(true); return value;
    }
    private static Method method(Class<?> type, String name, Class<?>... args) throws Exception {
        Method value = type.getDeclaredMethod(name, args); value.setAccessible(true); return value;
    }
}
