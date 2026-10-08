package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 切换输入方案时一起写进共享偏好的那几个键。设置页的方案选择、引导页的方案一步都走这里，所以两处写出的东西完全相同。
 *
 * <p>纯 Java，不依赖 androidx（check-host 直接编译它）。要写的值由 {@link #schemeValues} 算出，不碰 JSON，冒烟测试只能在 JVM 里测这一部分：check-host 的 classpath 里只有 android.jar 的 `org.json` 桩。
 */
public final class SchemePreferences {
    private SchemePreferences() {}

    /** Resolve the keyboard scheme from one preference snapshot using the shared fallback contract. */
    public static KeyboardScheme storedScheme(JSONObject preferences, AppEdition edition) {
        String scheme = InputViewValuePolicy.textOr(preferences, "scheme", edition.defaultScheme());
        String profile = InputViewValuePolicy.textOr(preferences, "shuangpin_profile", "xiaohe");
        String touchLayout = InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", "twenty_six_key");
        return KeyboardScheme.fromPreferences(scheme, profile, touchLayout, edition);
    }

    /**
     * 切到 `scheme` 时要写的偏好键和值，按写入顺序：`scheme`、`last_chinese_scheme`、`shuangpin_profile`、`touch_keyboard_layout`，`wubiProfile` 不为 null 时再加上规范化后的 `wubi_profile`。
     *
     * @param lastChineseScheme 偏好里现在的 `last_chinese_scheme`
     * @param shuangpinProfile 偏好里现在的 `shuangpin_profile`
     * @param wubiProfile 要写的五笔版本；null 表示保留现有版本
     */
    public static Map<String, String> schemeValues(KeyboardScheme scheme, String lastChineseScheme,
            String shuangpinProfile, String wubiProfile, AppEdition edition) {
        KeyboardScheme.PreferenceMapping mapping = scheme.mapping(lastChineseScheme, shuangpinProfile, edition);
        Map<String, String> values = new LinkedHashMap<>(5);
        values.put("scheme", mapping.scheme());
        values.put("last_chinese_scheme", mapping.lastChineseScheme());
        values.put("shuangpin_profile", mapping.shuangpinProfile());
        values.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
        if (wubiProfile != null) values.put("wubi_profile", KeyboardScheme.normalizedWubiProfile(wubiProfile));
        return values;
    }

    /**
     * 键盘自己的方案列表 `touch_keyboard_schemes.enabled` 在切到 `preferenceId` 之后的样子：已经列出就原样保留，没列出就追加到末尾。null 项原样保留，它们不是这次切换要清理的东西。
     *
     * @param enabled 现在的列表，项可以为 null；列表本身为 null 时当作空列表
     */
    public static List<String> enabledAfterSwitch(List<String> enabled, String preferenceId) {
        List<String> result = enabled == null
            ? new ArrayList<>(1) : new ArrayList<>(enabled.size() + 1);
        if (enabled != null) result.addAll(enabled);
        if (!result.contains(preferenceId)) result.add(preferenceId);
        return result;
    }

    /**
     * 偏好快照切到一个方案后的副本：{@link #schemeValues} 的那几个键，以及键盘方案列表存在时它的 `selected`。版本取当前包的 {@link AppEdition#current()}。
     *
     * @param wubiProfile 要写的五笔版本；null 表示保留现有版本，切到五笔时沿用用户上次选的 86 或 98
     * @return 改好的副本；快照为 null 或里面没有 preferences 对象时为 null
     */
    public static JSONObject withScheme(JSONObject snapshot, KeyboardScheme scheme, String wubiProfile) {
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        if (preferences == null) return null;
        AppEdition edition = AppEdition.current();
        Map<String, String> changes = schemeValues(scheme,
            InputViewValuePolicy.textOr(preferences, "last_chinese_scheme", edition.defaultScheme()),
            InputViewValuePolicy.textOr(preferences, "shuangpin_profile", "xiaohe"), wubiProfile, edition);
        try {
            JSONObject pending = new JSONObject(snapshot.toString());
            JSONObject values = pending.getJSONObject("preferences");
            for (Map.Entry<String, String> change : changes.entrySet()) values.put(change.getKey(), change.getValue());
            // 键盘自己的选择器一旦写过方案列表，它的 `selected` 在键盘决定显示哪个方案时优先于 `scheme`（KeyboardScheme.resolveEnabledSelection），所以这里切换也要改它，列表里没有这个方案时顺手启用。没有列表时键盘只看 `scheme`，不要新建列表。
            JSONObject schemes = values.optJSONObject("touch_keyboard_schemes");
            if (schemes != null) {
                JSONArray enabled = schemes.optJSONArray("enabled");
                List<String> current = new ArrayList<>(enabled == null ? 0 : enabled.length());
                if (enabled != null) {
                    for (int index = 0; index < enabled.length(); index++) {
                        current.add(enabled.isNull(index) ? null : enabled.optString(index, null));
                    }
                }
                List<String> next = enabledAfterSwitch(current, scheme.preferenceId());
                if (enabled == null) {
                    enabled = new JSONArray();
                    schemes.put("enabled", enabled);
                }
                for (int index = current.size(); index < next.size(); index++) enabled.put(next.get(index));
                schemes.put("selected", scheme.preferenceId());
            }
            return pending;
        } catch (JSONException error) {
            return null;
        }
    }
}
