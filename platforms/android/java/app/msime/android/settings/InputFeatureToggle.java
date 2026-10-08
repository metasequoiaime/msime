package app.msime.android;

import java.util.List;

/**
 * 设置页里那些一个布尔值就说得清的输入功能。
 *
 * <p>Each entry names the shared preference key it writes and the default the store applies when
 * the key is absent. The default matters as much as the key: a switch that renders unchecked for a
 * preference that is on by default tells the user the opposite of what the keyboard is doing, and
 * then turning it "on" writes a value that was already there.
 *
 * <p>Grouped rather than listed flat because the sheets they appear in are grouped, and a toggle
 * that moves between sheets should move by changing its group here.
 *
 * <p>Only preferences that really are one top-level boolean belong here. 自动纠错 is the two nested `quanpin` fields, both off by default, built by hand in the sheet because nesting is not something this table can express.
 */
public enum InputFeatureToggle {
    LEARNING(Group.DICTIONARY, "learning", true, "记忆新词",
        "把你选过的词排到前面；只在本机学习"),
    CLOUD_CANDIDATES(Group.DICTIONARY, "cloud_candidates", false, "云候选",
        "向服务端请求长句与新词，需要联网"),
    ENGLISH_SUGGESTIONS(Group.DICTIONARY, "english_suggestions", true, "英文联想",
        "英文模式下补全单词"),
    SINGLE_CHARACTER_ONLY(Group.DICTIONARY, "single_character_only", false, "只出单字",
        "候选只列单个汉字，不出词组和整句；选一个字后接着拼下一个字"),

    CHINESE_PUNCTUATION(Group.OUTPUT, "chinese_punctuation", true, "中文标点",
        "中文模式下把逗号句号打成全角"),
    SMART_PUNCTUATION(Group.OUTPUT, "smart_punctuation", true, "智能标点",
        "根据前一个字符决定标点形态"),
    PAIRED_PUNCTUATION(Group.OUTPUT, "paired_punctuation", true, "成对标点",
        "输入前引号、前括号时补上后半个"),
    TRADITIONAL_OUTPUT(Group.OUTPUT, "traditional_chinese_output", false, "繁体输出",
        "上屏前把简体转成繁体，不改变词库"),

    CLIPBOARD_HISTORY(Group.PRIVACY, "clipboard_history", false, "剪贴板历史",
        "在键盘里保留最近复制的内容，仅存本机；关闭会立即清空"),
    CANDIDATE_TRANSLATIONS(Group.PRIVACY, "candidate_translations", true, "候选翻译",
        "为候选词附上译文；联网翻译需同时开启下方的水杉账号翻译"),
    CANDIDATE_TRANSLATION_ACCOUNT(Group.PRIVACY, "translation_account", false, "用水杉账号翻译候选",
        "把当前页的中文候选词发送到 api.msime.app 翻译，首次使用会创建匿名账号；不开启则不联网翻译"),
    CANDIDATE_ENGLISH_GLOSS(Group.PRIVACY, "candidate_english_gloss", false, "候选英文释义",
        "用本机的离线词典给候选词标注释义；英语以外的目标语言要先下载离线释义词典"),
    USAGE_REPORTING(Group.PRIVACY, "usage_reporting", true, "匿名使用统计",
        "向 api.msime.app 发送当天是否使用、键盘会话是否正常结束和崩溃摘要，附应用版本与随机安装编号；不含输入内容和账号。关闭后立即停止并清空待发数据");

    /** Which sheet an entry belongs to. */
    public enum Group {
        DICTIONARY("词库与联想"),
        OUTPUT("上屏与标点"),
        PRIVACY("需要留意的");

        private final String title;

        Group(String title) { this.title = title; }

        public String title() { return title; }
    }

    private final Group group;
    private final String key;
    private final boolean enabledByDefault;
    private final String title;
    private final String description;

    InputFeatureToggle(Group group, String key, boolean enabledByDefault, String title,
            String description) {
        this.group = group;
        this.key = key;
        this.enabledByDefault = enabledByDefault;
        this.title = title;
        this.description = description;
    }

    public Group group() { return group; }

    /** The shared preference key this switch reads and writes. */
    public String key() { return key; }

    /** What the shared store applies when the key is absent from a snapshot. */
    public boolean enabledByDefault() { return enabledByDefault; }

    public String title() { return title; }

    public String description() { return description; }

    // Plain loops rather than streams: `Stream#toList` arrived in API 34 and this host declares
    // minSdk 28, so it compiles here and throws on most of the devices it ships to.

    /** The entries of one group, in declaration order. */
    public static List<InputFeatureToggle> of(Group group) {
        List<InputFeatureToggle> entries = new java.util.ArrayList<>(values().length);
        for (InputFeatureToggle value : values()) {
            if (value.group == group) entries.add(value);
        }
        return List.copyOf(entries);
    }

    /** Every group that has at least one entry, in declaration order. */
    public static List<Group> groups() {
        List<Group> groups = new java.util.ArrayList<>(Group.values().length);
        for (Group group : Group.values()) {
            if (!of(group).isEmpty()) groups.add(group);
        }
        return List.copyOf(groups);
    }
}
