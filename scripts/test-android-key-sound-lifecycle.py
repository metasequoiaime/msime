#!/usr/bin/env python3
"""执行完整 ImeKeyFeedback，验证来源切换与迟到加载不会串用音效。"""
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / 'platforms/android/java/app/msime/android'

# 只替换设备边界和非本测试职责的界面依赖；被测类使用未经裁剪的产品源码。
STUBS = {
    'android/content/Context.java': '''package android.content;
public class Context { public static final String AUDIO_SERVICE = "audio"; }
''',
    'android/graphics/Color.java': '''package android.graphics;
public class Color { public static int parseColor(String value) { return 0; } }
''',
    'android/view/ViewParent.java': '''package android.view; public interface ViewParent {}''',
    'android/view/View.java': '''package android.view;
public class View implements ViewParent {
    public ViewParent getParent() { return null; }
    public CharSequence getContentDescription() { return null; }
}
''',
    'android/media/AudioAttributes.java': '''package android.media;
public class AudioAttributes {
    public static final int USAGE_ASSISTANCE_SONIFICATION = 1, CONTENT_TYPE_SONIFICATION = 1;
    public static class Builder {
        public Builder setUsage(int value) { return this; }
        public Builder setContentType(int value) { return this; }
        public AudioAttributes build() { return new AudioAttributes(); }
    }
}
''',
    'android/media/AudioManager.java': '''package android.media;
public class AudioManager {
    public static final int FX_KEYPRESS_STANDARD = 1;
    public int plays;
    public void playSoundEffect(int effect, float volume) { plays++; }
}
''',
    'android/media/SoundPool.java': '''package android.media;
public class SoundPool {
    public static final java.util.List<SoundPool> created = new java.util.ArrayList<>();
    public int plays;
    public boolean released;
    public SoundPool() { created.add(this); }
    public int load(String path, int priority) { return 1; }
    public int play(int id, float left, float right, int priority, int loop, float rate) {
        if (released) throw new AssertionError("released pool played");
        plays++; return 1;
    }
    public void release() { released = true; }
    public static class Builder {
        public Builder setMaxStreams(int value) { return this; }
        public Builder setAudioAttributes(AudioAttributes value) { return this; }
        public SoundPool build() { return new SoundPool(); }
    }
}
''',
    'org/json/JSONException.java': '''package org.json;
public class JSONException extends Exception { private static final long serialVersionUID = 1; }
''',
    'org/json/JSONObject.java': '''package org.json;
public class JSONObject {
    public static final Object NULL = new Object();
    private static final java.util.Map<String, JSONObject> encoded = new java.util.HashMap<>();
    private final java.util.Map<String, Object> values = new java.util.HashMap<>();
    public JSONObject() {}
    public JSONObject(String text) throws JSONException {
        if (!encoded.containsKey(text)) throw new JSONException();
        values.putAll(encoded.get(text).values);
    }
    public JSONObject put(String key, Object value) throws JSONException { values.put(key, value); return this; }
    public Object opt(String key) { return values.get(key); }
    public JSONObject optJSONObject(String key) {
        Object value = values.get(key); return value instanceof JSONObject object ? object : null;
    }
    public boolean isNull(String key) { return opt(key) == null || opt(key) == NULL; }
    @Override public String toString() {
        String id = "synthetic-json-" + encoded.size(); encoded.put(id, this); return id;
    }
}
''',
    'app/msime/android/Environment.java': '''package app.msime.android;
import android.view.View;
import org.json.JSONObject;
class MSIMEInputService {
    JSONObject preferencesSnapshot;
    AndroidLocalSettings.Snapshot localSettings = new AndroidLocalSettings.Snapshot();
    String preferencesDirectory = "/synthetic/a";
    View spaceButton, enterButton, deleteButton, backspaceRepeatButton;
    View candidateViewport, expandedCandidates, keyRows, actionRow;
    final Main main = new Main();
    final android.media.AudioManager audio = new android.media.AudioManager();
    final Debug imeDebugOverlay = new Debug();
    final Rows imeLetterRows = new Rows();
    final Skin skin = new Skin();
    Object vibrator;
    Object hapticStrength;
    boolean soundEnabled = true, hapticsEnabled;
    int editorInputType;
    java.io.File getFilesDir() { return new java.io.File("/synthetic/files"); }
    Object getSystemService(String name) { return audio; }
    String keyIdOf(View view) { return null; }
}
class Main {
    final java.util.ArrayDeque<Runnable> tasks = new java.util.ArrayDeque<>();
    void post(Runnable task) { tasks.add(task); }
    void next() { tasks.remove().run(); }
}
class Debug { void recordEvent(Object event) {} void onKeyPressed(boolean value) {} }
class Rows { View keyPreviewLayer; }
class Skin { String accent() { return "#000000"; } }
class AndroidLocalSettings {
    static final String KEY_ANIMATION = "animation";
    static class Snapshot { String choice(String key) { return "none"; } }
}
class KeyboardPressButton extends View { KeyboardKeyRole keyboardRole() { return KeyboardKeyRole.RETURN; } }
enum KeyboardKeyRole { RETURN }
class DiagnosticEventKind { static final Object CANDIDATE_SELECTED = new Object(); }
class KeyPressAnimator {
    enum Style { NONE; static Style fromPreference(String value) { return NONE; } }
    static void play(View view, View layer, Style style, int color) {}
}
class KeyboardHaptics { static void play(Object vibrator, View view, Object strength) {} }
class EditorPolicy { static boolean password(int inputType) { return false; } }
class ImeLog { static void w(String message) {} }
class JsonPolicy {
    static String strictStringOrEmpty(Object raw) { return raw instanceof String value ? value : ""; }
    static String strictString(Object raw) { return strictStringOrEmpty(raw); }
    static JSONObject objectValue(JSONObject root) { return root.optJSONObject("value"); }
}
class BoundsPolicy { static int bounded(int value, int min, int max) { return Math.min(max, Math.max(min, value)); } }
class NumberPolicy { static int strictInt(Object value, int fallback) { return value instanceof Integer number ? number : fallback; } }
class NativeClient {
    static String sample;
    static final java.util.List<JSONObject> requests = new java.util.ArrayList<>();
    static String keySoundPack(String body) {
        try {
            requests.add(new JSONObject(body));
            return new JSONObject().put("value", new JSONObject().put("mode", "keys")
                .put("sounds", new JSONObject().put("default", sample))).toString();
        } catch (org.json.JSONException error) { throw new AssertionError(error); }
    }
}
''',
    'app/msime/android/policy/HostOptionsPolicy.java': '''package app.msime.android.policy;
public class HostOptionsPolicy {
    public static String bootstrapStateDirectory(java.io.File files) { return files + "/bootstrap/state"; }
    public static java.io.File soundPacksDirectory(java.io.File files) { return new java.io.File(files, "sound-packs"); }
}
''',
}

HARNESS = '''package app.msime.android;
import android.media.SoundPool;
import java.util.List;
import java.util.concurrent.AbstractExecutorService;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;
public class KeySoundLifecycleCheck {
    static final class Queue extends AbstractExecutorService {
        final java.util.ArrayDeque<Runnable> tasks = new java.util.ArrayDeque<>();
        boolean stopped;
        @Override public void execute(Runnable task) { tasks.add(task); }
        @Override public void shutdown() { stopped = true; }
        @Override public List<Runnable> shutdownNow() { shutdown(); return List.of(); }
        @Override public boolean isShutdown() { return stopped; }
        @Override public boolean isTerminated() { return stopped; }
        @Override public boolean awaitTermination(long time, TimeUnit unit) { return stopped; }
        void next() { tasks.remove().run(); }
    }
    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
    static void select(MSIMEInputService service, String pack) throws Exception {
        service.preferencesSnapshot = new JSONObject().put("preferences", new JSONObject().put("plugins",
            new JSONObject().put("key_sound", new JSONObject().put("pack", pack))));
    }
    public static void main(String[] args) throws Exception {
        NativeClient.sample = args[0];
        MSIMEInputService service = new MSIMEInputService();
        select(service, "synthetic-pack");
        ImeKeyFeedback player = new ImeKeyFeedback(service);
        Queue worker = new Queue();
        var loader = ImeKeyFeedback.class.getDeclaredField("loader");
        loader.setAccessible(true);
        ((java.util.concurrent.ExecutorService) loader.get(player)).shutdownNow();
        loader.set(player, worker);
        try {
            player.prepare(); worker.next(); service.main.next();
            JSONObject initial = NativeClient.requests.get(0);
            check("/synthetic/a".equals(initial.opt("state_root")), "initial source must reach native request");
            check("synthetic-pack".equals(initial.opt("pack")), "selected pack must reach native request");
            check("/synthetic/files/sound-packs".equals(initial.opt("sound_packs")), "bundled packs root must reach native request");
            SoundPool first = SoundPool.created.get(0);
            player.playFeedback(null);
            check(first.plays == 1, "loaded pool should play");
            player.prepare();
            check(worker.tasks.isEmpty(), "unchanged source must reuse its loaded pool");
            service.preferencesDirectory = "/synthetic/b";
            player.playFeedback(null);
            check(first.plays == 1, "directory-only change played samples from old directory");
            check(worker.tasks.size() == 1, "directory-only change must start a new load");
            worker.next(); service.main.next();
            check("/synthetic/b".equals(NativeClient.requests.get(1).opt("state_root")), "changed source must reach native request");
            check(first.released, "replaced pool must be released");
            // A 已缓存，B 加载完成但回调未投递时重新选择 A，B 不能覆盖它。
            SoundPool current = SoundPool.created.get(1);
            select(service, "synthetic-other"); player.prepare(); worker.next();
            SoundPool stale = SoundPool.created.get(2);
            select(service, "synthetic-pack"); player.prepare(); service.main.next();
            check(stale.released && !current.released, "late B load replaced the cached A after A-B-A");
            player.playFeedback(null);
            check(current.plays == 1, "cached A must remain playable after rejecting B");
            // 同一来源 A 先后两次请求：最早一次的回调不能冒充最新请求。
            select(service, "third-pack"); player.prepare();
            select(service, "fourth-pack"); player.prepare();
            select(service, "third-pack"); player.prepare();
            worker.next(); SoundPool oldest = SoundPool.created.get(3); service.main.next();
            check(oldest.released, "old request with the same pack and source must be rejected");
            worker.next(); service.main.next(); worker.next(); service.main.next();
            select(service, "fifth-pack"); player.prepare(); worker.next();
            player.shutdown(); service.main.next();
            for (SoundPool pool : SoundPool.created) check(pool.released, "shutdown must release all pools");
        } finally { player.shutdown(); }
        System.out.println("真实按键音宿主：目录切换、缓存 A-B-A、请求 A-B-A 与关闭释放通过");
    }
}
'''


def main():
    for tool in ('java', 'javac'):
        executable = shutil.which(tool)
        try:
            version = subprocess.run([executable, '-version'], capture_output=True,
                                     text=True, timeout=10) if executable else None
        except (OSError, subprocess.TimeoutExpired):
            version = None
        major = re.search(r'(?:javac|version)\s+"?(\d+)', version.stdout + version.stderr) if version else None
        if version is None or version.returncode != 0 or not major or int(major[1]) < 17:
            print('skipped: 按键音生命周期回归需要 JDK 17')
            return
    with tempfile.TemporaryDirectory(prefix='msime-key-sound-') as temporary:
        target = Path(temporary)
        sources = []
        for name, content in {**STUBS, 'app/msime/android/KeySoundLifecycleCheck.java': HARNESS}.items():
            path = target / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding='utf-8')
            sources.append(str(path))
        sample = target / 'synthetic.wav'
        sample.write_bytes(b'synthetic-sample')
        subprocess.run(['javac', '--release', '17', '-d', str(target),
                        str(JAVA / 'core/ImeKeyFeedback.java'),
                        str(JAVA / 'ThreadPolicy.java'),
                        str(JAVA / 'policy/KeySoundLifecyclePolicy.java'), *sources],
                       check=True, timeout=30)
        subprocess.run(['java', '-cp', str(target), 'app.msime.android.KeySoundLifecycleCheck', str(sample)],
                       check=True, timeout=15)


if __name__ == '__main__':
    main()
