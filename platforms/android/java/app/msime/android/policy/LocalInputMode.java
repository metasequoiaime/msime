package app.msime.android;

/** Apple-compatible local input mode shortcuts exposed by the shared Engine. */
public enum LocalInputMode {
    UNICODE("U", "Unicode 码点", "unicode"),
    DATE_TIME("T", "日期时间", "date_time"),
    SUPER_JIANPIN("J", "超级简拼", "super_jianpin"),
    QUICK_PHRASE("K", "快捷短语", "quick_phrase"),
    TEMPORARY_ENGLISH("Y", "英文补全", "temporary_english"),
    EMOJI("E", "表情", "emoji"),
    KAOMOJI("M", "颜文字", "kaomoji"),
    TEMPORARY_JAPANESE("R", "临时日语", "temporary_japanese"),
    // 拆字反查：输入字的各个部件的拼音找不会读的字（huohuohuo 得 焱）。不在 Apple 的参考里，追加在末尾。
    CHAIZI("C", "拆字", "chaizi"),
    // `/` 指令：内置指令和插件页里启用的指令表（`plugins.command_tables`）。`local_modes.command` 出厂关着（打开前 `/` 照常是一个标点），所以缺省读作关；Engine 只在中文标点下、没有组字时把 `/` 当作入口，宿主按同一个条件给这一项置灰（{@link #command}）。
    COMMAND("/", "指令", "command", false);

    private final String trigger;
    private final String title;
    private final String preferenceKey;
    private final boolean enabledByDefault;

    LocalInputMode(String trigger, String title, String preferenceKey) {
        this(trigger, title, preferenceKey, true);
    }

    LocalInputMode(String trigger, String title, String preferenceKey, boolean enabledByDefault) {
        this.trigger = trigger;
        this.title = title;
        this.preferenceKey = preferenceKey;
        this.enabledByDefault = enabledByDefault;
    }

    public String trigger() { return trigger; }
    public String title() { return title; }
    public String preferenceKey() { return preferenceKey; }
    /** 偏好 `local_modes` 里没有这一项时它开不开：与 client-core 的 `LocalModePreferences` 默认值一致。 */
    public boolean enabledByDefault() { return enabledByDefault; }
    /** 入口是 `/` 这个标点，不是 Shift+字母：按的时候不带 Shift，中文标点关着时 Engine 会把它当成普通的 `/`。 */
    public boolean command() { return this == COMMAND; }
}
