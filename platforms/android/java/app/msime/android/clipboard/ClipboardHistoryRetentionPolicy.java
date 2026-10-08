package app.msime.android;

/**
 * 剪贴板历史开关从哪一份偏好读，以及什么时候因为它关着而清空历史。
 *
 * <p>键盘手里有两份偏好。一份是 `runtime-options.json` 里的副本，首次安装时写下，之后再没更新过（见 Bootstrap.prepare），所以它的 `clipboard_history` 永远是出厂默认的关；另一份是共享存储里实时读到的。原先两份都会把开关写进服务，读到「关」就当场清空历史，而 `onStartInput` 每换一个输入框都先应用那份副本，`onCreateInputView` 又在实时偏好还没到时按字段初始值（关）清一次：于是每点一个输入框、每切一次输入法，历史就被清掉，只剩打开面板时补读回来的当前那一条（#5602）。
 *
 * <p>规则因此只有一条：只有实时读到的偏好能改变开关、能触发清空；副本和「还没读到」都不算数，开关保持原样。用户在设置里关掉开关后，键盘下一次实时读偏好时照样清空，「关闭会立即清空」的承诺不变。
 */
public final class ClipboardHistoryRetentionPolicy {
    /** 一份偏好从哪里来。服务应用偏好时由调用处明说（`MSIMEInputService.applyEditorPreferences`），同一个来源也决定要不要用它重算皮肤。 */
    public enum Source {
        /** `runtime-options.json` 里首次安装时写下的副本，内容是出厂默认，不代表用户的选择。 */
        RUNTIME_OPTIONS_COPY,
        /** 从共享偏好存储实时读到的那一份。 */
        LIVE
    }

    private ClipboardHistoryRetentionPolicy() {}

    /**
     * 应用一份偏好之后开关应该是什么。
     *
     * @param preference 这份偏好里的 `clipboard_history`；读不到偏好时为 null
     */
    public static boolean enabledAfter(boolean current, Source source, Boolean preference) {
        if (source != Source.LIVE || preference == null) return current;
        return preference;
    }

    /** 应用这份偏好时是否要清空历史：只有实时读到、并且明确是关的时候。 */
    public static boolean clearsHistory(Source source, Boolean preference) {
        return source == Source.LIVE && Boolean.FALSE.equals(preference);
    }
}
