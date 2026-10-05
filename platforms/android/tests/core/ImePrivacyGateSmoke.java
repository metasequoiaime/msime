package app.msime.android;

import android.text.InputType;
import android.view.inputmethod.EditorInfo;

/** 锁住 ImePrivacyGate 在拆分时的口径：与原来散在 MSIMEInputService 里的判断完全相同。 */
public final class ImePrivacyGateSmoke {
    public static void main(String[] arguments) {
        int text = InputType.TYPE_CLASS_TEXT;
        int password = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
        int numberPassword = InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD;
        int incognito = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
        check(!ImePrivacyGate.excludesKeyStatistics(text, 0), "a plain text field counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(password, 0), "a password field never counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(numberPassword, 0), "a numeric password never counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(text, incognito), "no personalised learning means no counting");
        check(ImePrivacyGate.excludesKeyStatistics(null), "no editor means no counting");
        for (int type : new int[] {text, password, numberPassword}) {
            for (int options : new int[] {0, incognito}) {
                check(ImePrivacyGate.excludesKeyStatistics(type, options)
                    == EditorPolicy.excludesKeyStatistics(type, options), "same answer as EditorPolicy");
            }
        }
        check(ImePrivacyGate.countsKeys(true, false), "switch on and editor allowed counts");
        check(!ImePrivacyGate.countsKeys(false, false), "switch off never counts");
        check(!ImePrivacyGate.countsKeys(true, true), "an excluded editor never counts");
        check(ImePrivacyGate.recordsTyping("/data/state", "你好"), "a commit with a directory is recorded");
        check(!ImePrivacyGate.recordsTyping("", "你好"), "no directory, nothing recorded");
        check(!ImePrivacyGate.recordsTyping("/data/state", ""), "an empty commit is not recorded");
        check(!ImePrivacyGate.recordsTyping("/data/state", null), "a missing commit is not recorded");
        check(ImePrivacyGate.capturesClipboard(true, true), "history on and store ready captures");
        check(!ImePrivacyGate.capturesClipboard(false, true), "history off never captures");
        check(!ImePrivacyGate.capturesClipboard(true, false), "no store, nothing captured");
        System.out.println("Android IME privacy gate passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
