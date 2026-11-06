package app.msime.android;

/**
 * 密码与具名终端触发的数字行，不修改本地偏好。
 *
 * <p>同一应用里换到普通输入框时保留（登录页的用户名、ConnectBot 的其他输入框）；进入别的应用的普通输入框、系统切换输入法、或用户把「顶部数字行」开关从开改成关时复位（#10878）。原先只在切换输入法时复位，进过一次验证码或密码框后，所有应用的 26 键都一直多一行，只能杀输入法进程才收得起来。
 */
public final class AutomaticNumberRow {
    private boolean automatic;
    /** 最近一次触发自动数字行的应用包名；不知道包名时为 null，此时任何普通输入框都算别的应用。 */
    private String triggerPackage;

    public void enterEditor(int inputType, String packageName) {
        if (EditorPolicy.password(inputType)
                || "org.connectbot".equals(packageName)
                || "com.sonelli.juicessh".equals(packageName)) {
            automatic = true;
            triggerPackage = packageName == null || packageName.isEmpty() ? null : packageName;
        } else if (automatic && (triggerPackage == null || !triggerPackage.equals(packageName))) {
            clear();
        }
    }

    public void inputMethodChanged() {
        clear();
    }

    /** 共享偏好 `touch_number_row` 重读后调用：用户把开关从开改成关，说明不想要这一行，留着的自动数字行一起收起；下次进入密码框或终端会重新开启。 */
    public void manualChanged(boolean previous, boolean next) {
        if (previous && !next) clear();
    }

    public boolean enabled(boolean manual) {
        return manual || automatic;
    }

    /** 当前编辑框触发了自动数字行；它的画法范围与共享偏好不同，见 {@link KeyboardLayout#drawsNumberRow(boolean, boolean, KeyboardLayout.Layer, int, int)}。 */
    public boolean active() {
        return automatic;
    }

    private void clear() {
        automatic = false;
        triggerPackage = null;
    }
}
