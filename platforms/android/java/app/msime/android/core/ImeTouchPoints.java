package app.msime.android;

import android.os.SystemClock;
import android.view.View;
import app.msime.android.core.InputViewValuePolicy;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.Arrays;

/**
 * 26 键双拼的按下点与键位几何（#10018，规则见 {@link TouchPointPolicy}）。按键区每次布局之后量一遍字母键（{@link LetterKeyMeasure}），和上次注册的不同、或者会话换了，就换一个代次号经 `msime_client_set_key_geometry` 注册给引擎；字母键被点击时取出它记下的按下点，条件都满足才交给 {@link MSIMEInputService#touchCharacter}。
 *
 * <p>开发者选项里「显示调试信息」打开时，把注册给引擎的几何 JSON 写到 `<filesDir>/debug/key-geometry.json`，用来拿真机的键位替换评测的参考布局。几何只是键的位置和大小，不含任何输入内容；按下点从不写出。
 */
final class ImeTouchPoints {
    static final String EXPORT_DIRECTORY = "debug";
    static final String EXPORT_FILE = "key-geometry.json";

    private final MSIMEInputService s;
    /** 按键区按键记下的按下点。 */
    final TouchPointPolicy pending = new TouchPointPolicy();
    private final LetterKeyMeasure keys = new LetterKeyMeasure();
    private final float[] point = new float[2];
    /** 当前会话注册的几何代次，0 是没注册（或注册被拒）。 */
    private int registeredId;
    private long registeredSession;
    /** 注册时量到的矩形，下次布局量出来一样就不再注册。 */
    private float[] registeredLetters = new float[0];
    private float[] registeredSemicolon;
    private float[] registeredDuplicates = new float[0];
    private char[] registeredDuplicateLetters = new char[0];
    /** 代次号只增不减，换会话也不回到 1，免得新会话碰巧对上旧的号。 */
    private int lastId;
    /** 最近一次注册的几何 JSON，和最近一次写出去的那份。 */
    private String exported;
    private String written;

    ImeTouchPoints(MSIMEInputService s) {
        this.s = s;
    }

    /** 当前会话注册着的几何代次，没有时为 0。 */
    int geometryId() {
        return registeredSession == s.session ? registeredId : 0;
    }

    /**
     * 按键区布局之后（{@link android.view.View.OnLayoutChangeListener}）或者点击时发现会话换了：显示的是 26 键字母层、26 个字母键都量得到时，几何和上次注册的不同就换代次重新注册。别的布局不注册也不清除，引擎只在 26 键双拼下用它，宿主也只在那时送按下点。
     */
    void register() {
        if (s.session == 0 || !(s.keyRows instanceof KeyboardKeyArea area) || !standardLetters()) return;
        if (keys.measure(area, s.shuangpinKeyButtons, s.shuangpinKeyInputs) != LetterKeyMeasure.LETTERS) return;
        float[] semicolon = keys.hasSemicolon ? keys.semicolon : null;
        float[] duplicates = Arrays.copyOf(keys.duplicates, keys.duplicateCount * 4);
        char[] duplicateLetters = Arrays.copyOf(keys.duplicateLetters, keys.duplicateCount);
        boolean same = registeredSession == s.session && registeredId != 0
            && Arrays.equals(registeredLetters, keys.letters)
            && Arrays.equals(registeredSemicolon, semicolon)
            && Arrays.equals(registeredDuplicates, duplicates)
            && Arrays.equals(registeredDuplicateLetters, duplicateLetters);
        if (same) {
            export(null);
            return;
        }
        lastId = lastId == Integer.MAX_VALUE ? 1 : lastId + 1;
        String json = TouchPointPolicy.geometryJson(lastId, keys.letters, semicolon,
            duplicateLetters, duplicates, duplicateLetters.length);
        registeredSession = s.session;
        registeredLetters = keys.letters.clone();
        registeredSemicolon = semicolon == null ? null : semicolon.clone();
        registeredDuplicates = duplicates;
        registeredDuplicateLetters = duplicateLetters;
        registeredId = s.registerKeyGeometry(json) ? lastId : 0;
        export(registeredId == 0 ? null : json);
    }

    /** 键行重建：旧键记下的按下点都作废。几何等新键布局之后再量。 */
    void forgetKeys() {
        pending.clear();
    }

    /** `source` 这个键被点击：取出并清掉它记下的按下点，写在 {@link #x()}、{@link #y()}。没有、过期时返回 false。`source` 为 null（不是 26 键字母键的点击）时什么都不取。 */
    boolean take(View source) {
        return source != null && pending.take(source, SystemClock.uptimeMillis(), point);
    }

    /** 取到按下点的这一键送出 `output`（`shifted` 是它带不带 Shift）时该不该带上按下点：该带时返回几何代次，否则返回 0，照常送 `msime_client_character`。会话换过而还没重新注册时先注册。 */
    int geometryFor(char output, boolean shifted) {
        if (registeredSession != s.session) register();
        int id = geometryId();
        String localMode = s.view == null ? "none" : InputViewValuePolicy.textOr(s.view, "local_mode", "none");
        boolean uses = TouchPointPolicy.usesTouch(standardLetters(), InputViewValuePolicy.scheme(s.view, -1),
            InputViewValuePolicy.textOr(s.view, "key_grid", "none"), s.dedicatedEnglish, localMode,
            output, shifted, id);
        return uses ? id : 0;
    }

    float x() {
        return point[0];
    }

    float y() {
        return point[1];
    }

    private boolean standardLetters() {
        return s.view != null && s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.STANDARD_TOUCH_LAYOUT;
    }

    /** 调试导出：开着「显示调试信息」时把注册的几何写出去；`json` 为 null 表示沿用上次注册的那份。写不进去时放弃，这是开发者自己打开的导出，不能影响打字。 */
    private void export(String json) {
        if (json != null) exported = json;
        if (exported == null || exported.equals(written) || !s.imeDebugOverlay.overlay()) return;
        File files = s.getFilesDir();
        if (files == null) return;
        File directory = new File(files, EXPORT_DIRECTORY);
        File target = new File(directory, EXPORT_FILE);
        File staging = new File(directory, EXPORT_FILE + ".tmp");
        try {
            Files.createDirectories(directory.toPath());
            Files.write(staging.toPath(), exported.getBytes(StandardCharsets.UTF_8));
            Files.move(staging.toPath(), target.toPath(), StandardCopyOption.REPLACE_EXISTING,
                StandardCopyOption.ATOMIC_MOVE);
            written = exported;
        } catch (IOException ignored) {
            // 调试导出失败不影响输入。
        }
    }
}
