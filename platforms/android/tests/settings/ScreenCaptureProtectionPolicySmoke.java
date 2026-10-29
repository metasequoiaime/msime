package app.msime.android;

import app.msime.android.ScreenCaptureProtectionPolicy.Decision;
import java.util.Map;

/** #10098：「防止截屏」第一次打开不认证，打开过以后再开或再关都要系统认证；没有系统认证时一律不切换；只在本机，不随同步和备份走。 */
public final class ScreenCaptureProtectionPolicySmoke {
    private static final int FLAG_SECURE = 0x00002000;

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void equal(Decision actual, Decision expected, String message) {
        if (actual != expected) throw new AssertionError(message + ": expected " + expected + ", got " + actual);
    }

    public static void main(String[] args) {
        decisions();
        windowFlags();
        storage();
        System.out.println("ScreenCaptureProtectionPolicySmoke ok");
    }

    private static void decisions() {
        equal(ScreenCaptureProtectionPolicy.decide(false, false, true, true), Decision.SAVE,
            "the first enable needs no authentication");
        equal(ScreenCaptureProtectionPolicy.decide(true, true, false, true), Decision.AUTHENTICATE,
            "turning protection off must require device authentication");
        equal(ScreenCaptureProtectionPolicy.decide(false, true, true, true), Decision.AUTHENTICATE,
            "turning protection on again after it was used must require device authentication");
        equal(ScreenCaptureProtectionPolicy.decide(true, false, false, true), Decision.AUTHENTICATE,
            "an enabled switch without the marker is still treated as used");
        equal(ScreenCaptureProtectionPolicy.decide(false, false, true, false), Decision.UNAVAILABLE,
            "without system authentication even the first enable is refused, or it could never be turned off");
        equal(ScreenCaptureProtectionPolicy.decide(true, true, false, false), Decision.UNAVAILABLE,
            "without system authentication protection stays on");
        equal(ScreenCaptureProtectionPolicy.decide(false, true, true, false), Decision.UNAVAILABLE,
            "without system authentication a used switch cannot be turned on again");
        for (boolean enabled : new boolean[] {false, true}) {
            for (boolean before : new boolean[] {false, true}) {
                for (boolean available : new boolean[] {false, true}) {
                    equal(ScreenCaptureProtectionPolicy.decide(enabled, before, enabled, available), Decision.UNCHANGED,
                        "keeping the current value does nothing");
                }
            }
        }
    }

    private static void windowFlags() {
        int other = 0x00000100 | 0x00000008;
        check(ScreenCaptureProtectionPolicy.windowFlags(other, true) == (other | FLAG_SECURE), "protection adds FLAG_SECURE");
        check(ScreenCaptureProtectionPolicy.windowFlags(other | FLAG_SECURE, false) == other,
            "turning protection off removes only FLAG_SECURE");
        check(ScreenCaptureProtectionPolicy.windowFlags(other, false) == other, "an unprotected window keeps its flags");
        check(ScreenCaptureProtectionPolicy.windowFlags(other | FLAG_SECURE, true) == (other | FLAG_SECURE),
            "an already protected window stays protected");
    }

    private static void storage() {
        AndroidLocalSettings.Snapshot defaults = AndroidLocalSettings.defaults();
        check(!defaults.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION), "protection is off by default");
        check(!defaults.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE), "a fresh install was never protected");
        for (String key : new String[] {AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION,
                AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE}) {
            check(!AndroidLocalSettings.spec(key).synced, key + " stays on this device");
            check(!LocalBackupPolicy.backsUpLocalSetting(key), key + " is neither exported nor restored by a backup");
        }
        AndroidLocalSettings.Snapshot enabled = new AndroidLocalSettings.Snapshot(AndroidLocalSettings.accepted(Map.of(
            AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION, true,
            AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE, true)));
        check(enabled.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION), "an explicit opt-in is read back");
        check(!enabled.synced().containsKey(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION), "the opt-in is not sent to sync");
        check(AndroidLocalSettings.accepted(Map.of(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION, "true")).isEmpty(),
            "a non-boolean value is dropped");
    }
}
