package app.msime.android;

/** 密码与具名终端触发的数字行，仅保留到系统切换输入法，不修改本地偏好。 */
public final class AutomaticNumberRow {
    private boolean automatic;

    public void enterEditor(int inputType, String packageName) {
        automatic |= EditorPolicy.password(inputType)
            || "org.connectbot".equals(packageName)
            || "com.sonelli.juicessh".equals(packageName);
    }

    public void inputMethodChanged() {
        automatic = false;
    }

    public boolean enabled(boolean manual) {
        return manual || automatic;
    }
}
