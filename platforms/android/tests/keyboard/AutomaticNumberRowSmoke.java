package app.msime.android;

import android.text.InputType;

/** 自动数字行的触发、跨输入框保留和输入法切换复位。 */
public final class AutomaticNumberRowSmoke {
    private static void check(boolean value, String label) {
        if (!value) throw new AssertionError(label);
    }

    public static void main(String[] args) {
        for (int type : new int[] {
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD,
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
                InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD}) {
            AutomaticNumberRow row = new AutomaticNumberRow();
            check(!row.enabled(false), "starts disabled");
            row.enterEditor(type, "app.example.synthetic");
            check(row.enabled(false), "password activates");
            row.enterEditor(InputType.TYPE_CLASS_TEXT, "app.example.other");
            check(row.enabled(false), "normal editor retains until IME switch");
            row.inputMethodChanged();
            check(!row.enabled(false), "IME switch clears automatic mode");
            check(row.enabled(true), "manual preference survives");
        }
        for (String terminal : new String[] {"org.connectbot", "com.sonelli.juicessh"}) {
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(0, terminal);
            check(row.enabled(false), "named terminal activates");
        }
        for (String other : new String[] {null, "", "org.connectbot.fake", "app.example.synthetic"}) {
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS, other);
            check(!row.enabled(false), "ordinary no-suggestions field is not a terminal");
        }
        AutomaticNumberRow row = new AutomaticNumberRow();
        row.enterEditor(InputType.TYPE_CLASS_NUMBER, "app.example.synthetic");
        check(!row.enabled(false), "ordinary number field has its existing number pad");
        row.inputMethodChanged();
        row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
        check(row.enabled(false), "returning to a password editor reactivates");
        check(!KeyboardLayout.drawsNumberRow(row.enabled(false), KeyboardLayout.Layer.LETTERS,
            KeyboardLayout.STANDARD_TOUCH_LAYOUT, 400), "height limit still applies");
        System.out.println("Automatic number row: context triggers, retention and IME reset passed");
    }
}
