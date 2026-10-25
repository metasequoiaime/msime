package app.msime.android.home;

import android.content.Context;
import android.media.AudioAttributes;
import android.media.SoundPool;
import android.os.Handler;
import android.os.Looper;
import app.msime.android.BoundsPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.NativeClient;
import app.msime.android.NumberPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import app.msime.android.policy.KeySoundPackPolicy;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 设置页里的按键音包：列出可选的包，选中后试听一声。包的根目录与键盘（`ImeKeyFeedback`）相同：APK 里解出的内置包 `<filesDir>/sound-packs`，与偏好目录下用户安装的包；包的校验只在 client-services 里做一次。
 */
final class KeySoundPacks {
    private KeySoundPacks() {}

    /** 插件目录里的按键音包；读不到时为空列表。读文件，要在工作线程上调用。 */
    static List<KeySoundPackPolicy.Pack> load(Context context) {
        List<KeySoundPackPolicy.Entry> entries = new ArrayList<>();
        try {
            JSONObject request = roots(context).put("action", new JSONObject().put("operation", "catalog"));
            JSONObject value = JsonPolicy.objectValue(new JSONObject(NativeClient.plugins(request.toString())));
            JSONArray packages = value == null ? null : value.optJSONArray("packages");
            for (int index = 0; packages != null && index < packages.length(); index++) {
                JSONObject pack = packages.optJSONObject(index);
                if (pack == null) continue;
                entries.add(new KeySoundPackPolicy.Entry(JsonPolicy.strictStringOrEmpty(pack.opt("id")),
                    JsonPolicy.strictStringOrEmpty(pack.opt("name")), JsonPolicy.strictStringOrEmpty(pack.opt("kind")),
                    JsonPolicy.strictStringOrEmpty(pack.opt("mode"))));
            }
        } catch (JSONException | RuntimeException | LinkageError error) {
            return List.of();
        }
        return KeySoundPackPolicy.choices(entries);
    }

    /**
     * 用 `pack` 的普通键样本试听一声，音量取偏好 `plugins.key_sound.volume`（0–100，异常值按 100）。播完自己释放；包读不到时不出声。读文件，要在工作线程上调用。
     */
    static void preview(Context context, String pack, Object volumePreference) {
        String sample;
        try {
            JSONObject request = roots(context).put("pack", pack);
            JSONObject value = JsonPolicy.objectValue(new JSONObject(NativeClient.keySoundPack(request.toString())));
            JSONObject sounds = value == null ? null : value.optJSONObject("sounds");
            sample = sounds == null ? "" : JsonPolicy.strictStringOrEmpty(sounds.opt("default"));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return;
        }
        if (sample.isEmpty() || !new File(sample).isFile()) return;
        float volume = BoundsPolicy.bounded(NumberPolicy.strictInt(volumePreference, 100), 0, 100) / 100f;
        // 与键盘同一种音频用途，静音和勿扰时同样不响。
        SoundPool pool = new SoundPool.Builder().setMaxStreams(1)
            .setAudioAttributes(new AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_ASSISTANCE_SONIFICATION)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
            .build();
        Handler main = new Handler(Looper.getMainLooper());
        pool.setOnLoadCompleteListener((loaded, id, status) -> {
            if (status == 0) loaded.play(id, volume, volume, 1, 0, 1f);
            // 样本上限 1.5 秒（client-services 校验），留够余量再释放。
            main.postDelayed(loaded::release, 2000);
        });
        if (pool.load(sample, 1) == 0) pool.release();
    }

    private static JSONObject roots(Context context) throws JSONException {
        File packs = HostOptionsPolicy.soundPacksDirectory(context.getFilesDir());
        return new JSONObject()
            .put("state_root", HostStore.directory(context))
            .put("sound_packs", packs == null ? JSONObject.NULL : packs.getAbsolutePath());
    }
}
