package app.msime.android;

import app.msime.android.AndroidLocalSettings;
import app.msime.android.KeyboardHeightSettingPolicy;
import java.util.Map;

public final class KeyboardHeightSettingPolicySmoke {
    public static void main(String[] args) {
        AndroidLocalSettings.Snapshot empty = AndroidLocalSettings.defaults();
        check(KeyboardHeightSettingPolicy.keyFor(599).equals(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT),
            "compact screens use the legacy height key");
        check(KeyboardHeightSettingPolicy.keyFor(600).equals(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT_LARGE),
            "expanded screens use the large height key");
        check(KeyboardHeightSettingPolicy.adjustment(empty, 599, 18) == 18,
            "compact screens use the shared fallback when no local value exists");
        check(KeyboardHeightSettingPolicy.adjustment(empty, 600, 18) == 18,
            "expanded screens use the shared fallback before a large value is saved");
        AndroidLocalSettings.Snapshot legacy = new AndroidLocalSettings.Snapshot(Map.of(
            AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT, 55));
        check(KeyboardHeightSettingPolicy.adjustment(legacy, 600, 18) == 55,
            "expanded screens inherit an existing legacy value");
        AndroidLocalSettings.Snapshot both = new AndroidLocalSettings.Snapshot(Map.of(
            AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT, 55,
            AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT_LARGE, -12));
        check(KeyboardHeightSettingPolicy.adjustment(both, 599, 18) == 55,
            "compact screens ignore the large value");
        check(KeyboardHeightSettingPolicy.adjustment(both, 600, 18) == -12,
            "expanded screens prefer their independent value");
        System.out.println("Android keyboard height form-factor policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
