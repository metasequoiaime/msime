package app.msime.android;

import android.text.InputType;

/** 自动数字行的触发、同一应用内跨输入框保留、换到别的应用或输入法切换时复位，以及手动关闭开关时复位（#10878）。 */
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
            row.enterEditor(InputType.TYPE_CLASS_TEXT, "app.example.synthetic");
            check(row.enabled(false), "ordinary editor in the same app retains");
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
        // #10878：验证码或密码框之后进别的应用，自动数字行不能一直留着。
        for (String next : new String[] {"app.example.other", null, ""}) {
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
            row.enterEditor(InputType.TYPE_CLASS_TEXT, next);
            check(!row.enabled(false), "ordinary editor in another app clears automatic mode");
            row.enterEditor(InputType.TYPE_CLASS_TEXT, "app.example.synthetic");
            check(!row.enabled(false), "returning to an ordinary editor in the trigger app stays cleared");
        }
        {
            // 没有包名时无法判断是不是同一应用，按别的应用处理。
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, null);
            row.enterEditor(InputType.TYPE_CLASS_TEXT, null);
            check(!row.enabled(false), "unknown package does not count as the same app");
        }
        {
            // 终端 → 别的应用的密码框 → 回到同一应用的普通输入框：保留到最近一次触发的应用。
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(0, "org.connectbot");
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
            check(row.enabled(false), "password in another app keeps automatic mode");
            row.enterEditor(InputType.TYPE_CLASS_TEXT, "app.example.synthetic");
            check(row.enabled(false), "latest trigger app owns the retention");
            row.enterEditor(0, "org.connectbot");
            check(row.enabled(false), "terminal reactivates by package");
        }
        {
            // 用户把「顶部数字行」从开改成关，同一应用里留着的自动数字行一起收起。
            AutomaticNumberRow row = new AutomaticNumberRow();
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
            row.enterEditor(InputType.TYPE_CLASS_TEXT, "app.example.synthetic");
            row.manualChanged(false, true);
            check(row.enabled(false), "turning the switch on keeps automatic mode");
            row.manualChanged(true, true);
            check(row.enabled(false), "unchanged switch keeps automatic mode");
            row.manualChanged(true, false);
            check(!row.enabled(false), "turning the switch off clears automatic mode");
            row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
            check(row.enabled(false), "password reactivates after the switch was turned off");
        }
        AutomaticNumberRow row = new AutomaticNumberRow();
        row.enterEditor(InputType.TYPE_CLASS_NUMBER, "app.example.synthetic");
        check(!row.enabled(false), "ordinary number field has its existing number pad");
        row.inputMethodChanged();
        row.enterEditor(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, "app.example.synthetic");
        check(row.enabled(false), "returning to a password editor reactivates");
        check(!KeyboardLayout.drawsNumberRow(row.enabled(false), KeyboardLayout.Layer.LETTERS,
            KeyboardLayout.STANDARD_TOUCH_LAYOUT, 400), "height limit still applies");
        System.out.println("Automatic number row: context triggers, same-app retention, app change, switch-off and IME reset passed");
    }
}
