package app.msime.android;

import app.msime.android.ClipboardDeleteAuthenticationPolicy.Decision;
import java.util.Map;

/** #10408：「剪贴板删除数据验证」打开和关闭都要系统认证（第一次打开也一样），没有系统认证时一律不切换；访客剪贴板不看开关，删除一律认证；只在本机，不随同步和备份走。 */
public final class ClipboardDeleteAuthenticationPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        decide();
        deletion();
        storage();
        System.out.println("ClipboardDeleteAuthenticationPolicySmoke ok");
    }

    private static void decide() {
        for (boolean enabled : new boolean[] {false, true}) {
            for (boolean available : new boolean[] {false, true}) {
                check(ClipboardDeleteAuthenticationPolicy.decide(enabled, enabled, available) == Decision.UNCHANGED,
                    "an unchanged value does nothing: enabled=" + enabled + " available=" + available);
            }
        }
        check(ClipboardDeleteAuthenticationPolicy.decide(false, true, true) == Decision.AUTHENTICATE,
            "the first switch-on authenticates too");
        check(ClipboardDeleteAuthenticationPolicy.decide(true, false, true) == Decision.AUTHENTICATE,
            "switching off authenticates");
        check(ClipboardDeleteAuthenticationPolicy.decide(false, true, false) == Decision.UNAVAILABLE,
            "without a lock-screen credential the switch cannot be turned on");
        check(ClipboardDeleteAuthenticationPolicy.decide(true, false, false) == Decision.UNAVAILABLE,
            "after the lock screen is removed the switch cannot be turned off either");
        check(ClipboardDeleteAuthenticationPolicy.refusal(Decision.UNAVAILABLE, true)
                .equals(ClipboardDeleteAuthenticationPolicy.UNAVAILABLE_MESSAGE),
            "the unavailable refusal points to the system lock-screen setting");
        check(ClipboardDeleteAuthenticationPolicy.UNAVAILABLE_MESSAGE.contains("锁屏"),
            "the guidance names the lock screen");
        check(ClipboardDeleteAuthenticationPolicy.refusal(Decision.AUTHENTICATE, true).contains("保持关闭"),
            "a failed switch-on says the switch stays off");
        check(ClipboardDeleteAuthenticationPolicy.refusal(Decision.AUTHENTICATE, false).contains("保持开启"),
            "a failed switch-off says the switch stays on");
    }

    private static void deletion() {
        check(!ClipboardDeleteAuthenticationPolicy.requiresAuthentication(false, false),
            "the main clipboard deletes without authentication while the switch is off");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(true, false),
            "the main clipboard authenticates while the switch is on");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(false, true),
            "the visitor clipboard authenticates even while the switch is off");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(true, true),
            "the visitor clipboard authenticates while the switch is on");
        int authenticated = ClipboardOperationAuthenticationPolicy.RESULT_AUTHENTICATED;
        int unavailable = ClipboardOperationAuthenticationPolicy.RESULT_UNAVAILABLE;
        // Activity.RESULT_CANCELED 是 0、RESULT_OK 是 -1：认证桥的两个结果码都不能与它们或彼此相同，否则「没有锁屏」会被当成取消或通过。
        check(authenticated != unavailable && authenticated != 0 && authenticated != -1
                && unavailable != 0 && unavailable != -1,
            "the bridge tells success, cancellation and a missing lock screen apart");
    }

    private static void storage() {
        String key = AndroidLocalSettings.CLIPBOARD_DELETE_AUTHENTICATION;
        check(!AndroidLocalSettings.defaults().bool(key), "the switch is off by default");
        check(!AndroidLocalSettings.spec(key).synced, "the switch stays on this device");
        check(!LocalBackupPolicy.backsUpLocalSetting(key), "the switch is neither exported nor restored by a backup");
        AndroidLocalSettings.Snapshot enabled = new AndroidLocalSettings.Snapshot(
            AndroidLocalSettings.accepted(Map.of(key, true)));
        check(enabled.bool(key), "a stored opt-in is read back after a restart");
        check(!enabled.synced().containsKey(key), "the opt-in is not sent to sync");
        check(AndroidLocalSettings.accepted(Map.of(key, "true")).isEmpty(), "a non-boolean value is dropped");
    }
}
