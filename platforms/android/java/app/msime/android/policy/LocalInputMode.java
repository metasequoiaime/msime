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
    CHAIZI("C", "拆字", "chaizi");

    private final String trigger;
    private final String title;
    private final String preferenceKey;

    LocalInputMode(String trigger, String title, String preferenceKey) {
        this.trigger = trigger;
        this.title = title;
        this.preferenceKey = preferenceKey;
    }

    public String trigger() { return trigger; }
    public String title() { return title; }
    public String preferenceKey() { return preferenceKey; }
}
