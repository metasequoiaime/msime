package app.msime.android;

import android.content.Context;
import android.graphics.Color;
import android.media.AudioAttributes;
import android.media.AudioManager;
import android.media.SoundPool;
import android.os.Build;
import android.os.VibrationEffect;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.ViewParent;
import java.io.File;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 按键反馈（P23）：按键音、振动与按键动画。
 *
 * <p>按键音开关仍是 Android 本地的 `KeyboardFeedbackStore`；声音包取共享偏好 `plugins.key_sound.pack`，音量取 `plugins.key_sound.volume`（0–100）。包是 `default`（或没选）时沿用系统的 `FX_KEYPRESS_STANDARD`；其他包经 `NativeClient.keySoundPack` 取到校验过的样本路径，用 `SoundPool` 按键类（其他键 / 空格 / 回车 / 退格）播放，密码框里不播。按键动画按 `touch_key_animation` 由 {@link KeyPressAnimator} 播放，`none` 保持原来的按压态。
 */
final class ImeKeyFeedback {
    /** 按键类别：普通字符键。 */
    static final int KEY_STANDARD = 0;
    /** 按键类别：空格。 */
    static final int KEY_SPACE = 1;
    /** 按键类别：回车。 */
    static final int KEY_ENTER = 2;
    /** 按键类别：退格。 */
    static final int KEY_BACKSPACE = 3;

    private final MSIMEInputService s;
    private final ExecutorService loader = Executors.newSingleThreadExecutor(runnable -> {
        Thread thread = new Thread(runnable, "msime-key-sound");
        thread.setDaemon(true);
        return thread;
    });
    private JSONObject preferencesSeen;
    private AndroidLocalSettings.Snapshot localSeen;
    private String packId = "default";
    private float volume = 1f;
    private KeyPressAnimator.Style animation = KeyPressAnimator.Style.NONE;
    /** 当前声音包加载出的样本；没有（default 包、加载中或失败）时为 null，此时用系统按键音。 */
    private volatile PackSounds sounds;
    private String loadingPack = "";

    ImeKeyFeedback(MSIMEInputService s) {
        this.s = s;
    }

    /** 一个声音包在 SoundPool 里的四个样本 id；0 表示该类没有样本，回落到普通键的样本。 */
    private record PackSounds(String id, SoundPool pool, int standard, int space, int enter, int backspace) {
        int sample(int keyClass) {
            int value = switch (keyClass) {
                case KEY_SPACE -> space;
                case KEY_ENTER -> enter;
                case KEY_BACKSPACE -> backspace;
                default -> standard;
            };
            return value != 0 ? value : standard;
        }
    }

    /** 一次按键的反馈入口。 */
    void onKeyDown(View key, int keyClass) {
        feedback(key, keyClass);
    }

    void playFeedback(View source) {
        feedback(source, classify(source));
    }

    /** 由按键视图推断按键类别：空格键、回车键、删除键各自一类，其他都算普通键。 */
    int classify(View source) {
        if (source == null) return KEY_STANDARD;
        if (source == s.spaceButton) return KEY_SPACE;
        if (source == s.enterButton) return KEY_ENTER;
        if (source == s.deleteButton || source == s.backspaceRepeatButton) return KEY_BACKSPACE;
        if (source instanceof KeyboardPressButton press && press.keyboardRole() == KeyboardKeyRole.RETURN)
            return KEY_ENTER;
        CharSequence description = source.getContentDescription();
        if (description != null) {
            String value = description.toString();
            if ("删除".equals(value)) return KEY_BACKSPACE;
            if ("空格".equals(value)) return KEY_SPACE;
        }
        return KEY_STANDARD;
    }

    /** 偏好里的声音包 id；空值按 `default`。 */
    static String packFor(JSONObject preferences) {
        JSONObject plugins = preferences == null ? null : preferences.optJSONObject("plugins");
        JSONObject keySound = plugins == null ? null : plugins.optJSONObject("key_sound");
        String pack = keySound == null ? "" : keySound.optString("pack", "");
        return pack == null || pack.isEmpty() ? "default" : pack;
    }

    /** 0–100 的音量换成 SoundPool 的 0–1。 */
    static float volumeFor(int percent) {
        return Math.max(0, Math.min(100, percent)) / 100f;
    }

    private void refreshPreferences() {
        JSONObject preferences = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        if (preferences == preferencesSeen && s.localSettings == localSeen) return;
        preferencesSeen = preferences;
        localSeen = s.localSettings;
        packId = packFor(preferences);
        JSONObject plugins = preferences == null ? null : preferences.optJSONObject("plugins");
        JSONObject keySound = plugins == null ? null : plugins.optJSONObject("key_sound");
        volume = volumeFor(keySound == null ? 100 : keySound.optInt("volume", 100));
        animation = KeyPressAnimator.Style.fromPreference(
            s.localSettings.choice(AndroidLocalSettings.KEY_ANIMATION));
        PackSounds loaded = sounds;
        if ("default".equals(packId)) {
            if (loaded != null) release(loaded);
            sounds = null;
            loadingPack = "";
        } else if ((loaded == null || !loaded.id().equals(packId)) && !packId.equals(loadingPack)) {
            load(packId);
        }
    }

    private void feedback(View source, int keyClass) {
        refreshPreferences();
        if (inside(source, s.candidateViewport) || inside(source, s.expandedCandidates)) {
            s.imeDebugOverlay.recordEvent(ImeDebugOverlay.Event.CANDIDATE_SELECTED);
        } else if (inside(source, s.keyRows) || inside(source, s.actionRow)) {
            s.imeDebugOverlay.onKeyPressed(keyClass == KEY_BACKSPACE);
            if (animation != KeyPressAnimator.Style.NONE) {
                KeyPressAnimator.play(source, s.imeLetterRows.keyPreviewLayer, animation,
                    Color.parseColor(s.skin.accent()));
            }
        }
        if (s.soundEnabled) playSound(keyClass);
        if (!s.hapticsEnabled) return;
        if (Build.VERSION.SDK_INT >= 26 && s.vibrator != null && s.vibrator.hasVibrator()) {
            s.vibrator.vibrate(VibrationEffect.createOneShot(10, s.hapticStrength.amplitude()));
        } else {
            source.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
        }
    }

    private void playSound(int keyClass) {
        PackSounds loaded = sounds;
        if (!"default".equals(packId) && loaded != null && loaded.id().equals(packId)) {
            // 密码框里不播声音包：不同键的不同声音会泄露按了哪些键。
            if (EditorPolicy.password(s.editorInputType)) return;
            int sample = loaded.sample(keyClass);
            if (sample != 0) {
                loaded.pool().play(sample, volume, volume, 1, 0, 1f);
                return;
            }
        }
        AudioManager audio = (AudioManager) s.getSystemService(Context.AUDIO_SERVICE);
        if (audio != null) audio.playSoundEffect(AudioManager.FX_KEYPRESS_STANDARD);
    }

    private static boolean inside(View view, View ancestor) {
        if (view == null || ancestor == null) return false;
        if (view == ancestor) return true;
        ViewParent parent = view.getParent();
        while (parent instanceof View node) {
            if (node == ancestor) return true;
            parent = node.getParent();
        }
        return false;
    }

    /** 声音包所在的两个根：APK 里解出的内置包 `<filesDir>/sound-packs`，与用户安装的包所在的状态目录。 */
    private String request(String pack) throws JSONException {
        File files = s.getFilesDir();
        String state = s.preferencesDirectory != null && new File(s.preferencesDirectory).isAbsolute()
            ? s.preferencesDirectory
            : files == null ? null : new File(files, "bootstrap/state").getAbsolutePath();
        JSONObject request = new JSONObject();
        request.put("state_root", state == null ? JSONObject.NULL : state);
        request.put("sound_packs", files == null ? JSONObject.NULL
            : new File(files, "sound-packs").getAbsolutePath());
        request.put("pack", pack);
        return request.toString();
    }

    private void load(String pack) {
        loadingPack = pack;
        final String body;
        try {
            body = request(pack);
        } catch (JSONException error) {
            loadingPack = "";
            return;
        }
        try {
            loader.execute(() -> {
                PackSounds next = null;
                try {
                    JSONObject root = new JSONObject(NativeClient.keySoundPack(body));
                    JSONObject value = root.optBoolean("ok", false) ? root.optJSONObject("value") : null;
                    JSONObject files = value == null ? null : value.optJSONObject("sounds");
                    if (value != null && "keys".equals(value.optString("mode")) && files != null) {
                        SoundPool pool = new SoundPool.Builder().setMaxStreams(4)
                            .setAudioAttributes(new AudioAttributes.Builder()
                                .setUsage(AudioAttributes.USAGE_ASSISTANCE_SONIFICATION)
                                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
                            .build();
                        next = new PackSounds(pack, pool, sample(pool, files, "default"),
                            sample(pool, files, "space"), sample(pool, files, "enter"),
                            sample(pool, files, "backspace"));
                        if (next.standard() == 0) {
                            pool.release();
                            next = null;
                        }
                    }
                } catch (JSONException | RuntimeException | LinkageError error) {
                    ImeLog.w("Key sound pack unavailable");
                    next = null;
                }
                PackSounds loaded = next;
                s.main.post(() -> {
                    if (!pack.equals(loadingPack)) {
                        if (loaded != null) release(loaded);
                        return;
                    }
                    loadingPack = "";
                    PackSounds previous = sounds;
                    sounds = loaded;
                    if (previous != null && previous != loaded) release(previous);
                });
            });
        } catch (RejectedExecutionException error) {
            loadingPack = "";
        }
    }

    private static int sample(SoundPool pool, JSONObject files, String name) {
        if (files.isNull(name)) return 0;
        String path = files.optString(name, "");
        if (path.isEmpty() || !new File(path).isFile()) return 0;
        return pool.load(path, 1);
    }

    private static void release(PackSounds loaded) {
        try {
            loaded.pool().release();
        } catch (RuntimeException ignored) {
            // 已经释放过。
        }
    }
}
