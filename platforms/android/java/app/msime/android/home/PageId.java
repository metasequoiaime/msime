package app.msime.android.home;

import androidx.annotation.Nullable;
import app.msime.android.HostDeepLink;
import app.msime.android.ListPolicy;
import app.msime.android.TextPolicy;
import java.util.Arrays;
import java.util.List;

/**
 * 宿主全部详情页的注册表：每一项是页面 Fragment 的全限定类名、标题、所属 tab 和设置首页搜索用的关键词。
 *
 * <p>页面按类名注册而不是按类引用，是为了让本表先于页面存在：页面类由后续切片逐个新建，还不存在的页面由 {@link SettingsNavigator} 提示「即将推出」，而不是让本表编译不过。深链只按枚举名（`PageId.valueOf`）映射到这里，从不接受 Intent 给出的类名。
 *
 * <p>页面类的约束见 {@link DetailPage}：必须是 public 类、有 public 无参构造器、参数只放在 arguments Bundle 里。
 *
 * <p>关键词取自对应设计图上的行标题，搜索时与标题一起匹配；{@link #LEXICON_DETAIL} 需要参数才能打开，所以没有关键词，不出现在搜索结果里。页面里新加的开关、导航和滑块行要同时把行标题加进这里，否则设置首页搜不到它（#6132 的「浮动键盘」就是这样漏掉的）；`tests/home/SettingsSearchIndexSmoke.java` 逐页核对。
 */
public enum PageId {
    SKINS("SkinsPage", "皮肤", HostDeepLink.TAB_SETTINGS,
        "跟随系统", "水杉四季", "水杉", "浅色", "纸白", "夜青", "墨", "春芽", "夏荫", "秋杉", "冬雪", "AI 设计皮肤"),
    AI_SKIN("AiSkinPage", "AI 设计皮肤", HostDeepLink.TAB_SETTINGS,
        "描述一句话生成", "配色", "按键音效", "按键动画", "生成皮肤"),
    KEYBOARD_OPTIONS("KeyboardOptionsPage", "键盘", HostDeepLink.TAB_SETTINGS,
        "布局", "中文键盘", "中英键轮换其他语言", "九键左侧符号", "九键数字键盘左侧符号", "数字键盘顺序", "双拼键位提示", "键盘高度", "横屏分离式键盘",
        "浮动键盘", "键盘底栏", "按键反馈", "按键音", "按键振动", "按键弹出预览", "按键动画", "手势",
        "滑动输入符号", "滑动方向", "下滑", "上滑", "九键滑动输入数字", "滑行输入", "空格键滑动移动光标", "长按空格语音输入", "键盘工具栏", "显示方式", "表情", "常用语",
        "皮肤", "剪贴板", "输入方式", "文本编辑", "按键间距", "行间距", "剪贴板排列", "单列", "双列", "工具栏显示最近复制", "最近复制"),
    AI_SETTINGS("AiSettingsPage", "AI 润色与回复", HostDeepLink.TAB_SETTINGS,
        "启用 AI 入口", "端点 URL", "模型", "凭据", "润色提示词"),
    TYPING("TypingPage", "输入", HostDeepLink.TAB_SETTINGS,
        "语言与方案", "普通话", "粤语", "英语", "日语", "韩语", "越南语", "藏语", "添加语言", "中文", "中文字符集", "简体", "繁体",
        "拼音纠错", "模糊音", "只出单字", "云候选", "辅助码", "启用辅助码", "辅助码方案", "辅助码模式", "翻译", "候选词翻译",
        "离线英文释义", "翻译目标语言"),
    EXPRESSION("ExpressionPage", "表达", HostDeepLink.TAB_SETTINGS,
        "标点", "使用英文标点", "自动补全成对标点", "智能标点", "智能", "整句联想", "英文自动纠正", "英文联想",
        "候选带 emoji", "候选带颜文字", "发现短语"),
    LEXICON("LexiconPage", "词库", HostDeepLink.TAB_SETTINGS,
        "已安装", "拼音词库", "新建词库", "导入词库", "导出", "发现词库", "记忆新词", "学习", "背单词", "云词库"),
    LEXICON_DETAIL("LexiconDetailPage", "词库详情", HostDeepLink.TAB_SETTINGS),
    PHRASES("PhrasesPage", "常用语", HostDeepLink.TAB_SETTINGS,
        "添加常用语", "修改常用语", "删除常用语"),
    VOICE("VoicePage", "语音输入", HostDeepLink.TAB_SETTINGS,
        "识别", "识别语言", "自动添加标点", "离线识别", "启动方式", "长按空格", "隐私", "上传语音以改进识别"),
    HANDWRITING("HandwritingPage", "手写输入", HostDeepLink.TAB_SETTINGS,
        "书写", "书写模式", "叠写", "识别等待时间", "识别后显示拼音", "笔迹", "笔迹颜色", "笔迹粗细"),
    DEVELOPER("DeveloperPage", "开发者选项", HostDeepLink.TAB_SETTINGS,
        "MCP 开发者访问", "上传日志", "上传日志供开发者通过 MCP 读取", "保留时长", "可访问的日志", "崩溃日志", "性能日志", "输入事件", "配置快照",
        "调试", "显示调试信息", "记录输入日志", "日志级别", "数据", "导出诊断包", "重置所有设置"),
    FEEDBACK("FeedbackPage", "帮助与反馈", HostDeepLink.TAB_ACCOUNT,
        "反馈", "反馈类型", "附带诊断信息", "描述", "添加截图", "提交", "复制设备信息"),
    ABOUT("AboutPage", "关于", HostDeepLink.TAB_ACCOUNT,
        "版本", "检查更新", "更新", "自动更新", "更新通道", "稳定版", "设备信息", "复制设备信息", "官网", "法律信息",
        "用户协议", "隐私政策", "开源许可", "给我们评分", "在管理界面中查看"),
    HELP("HelpPage", "使用帮助", HostDeepLink.TAB_ACCOUNT,
        "启用键盘", "打开系统键盘设置", "打字", "选择候选词", "换一种输入方案", "换皮肤与布局", "遇到问题",
        "完整文档", "反馈问题与建议"),
    DOWNLOAD("DownloadPage", "其他平台下载", HostDeepLink.TAB_ACCOUNT,
        "在电脑上打开", "复制链接", "电脑", "发送链接", "Windows", "macOS", "Linux", "HarmonyOS", "手机和平板",
        "iOS", "iPadOS", "Android"),
    CLOUD_CLIPBOARD("CloudClipboardPage", "云剪贴板", HostDeepLink.TAB_ACCOUNT,
        "保留时长", "最近", "清空"),
    PROFILE("ProfilePage", "个人资料", HostDeepLink.TAB_ACCOUNT,
        "账号", "昵称", "水杉 ID", "邮箱", "登录方式", "关联", "云端数据", "导出我的数据", "退出登录", "注销账号"),
    BACKUP("BackupPage", "备份与恢复", HostDeepLink.TAB_ACCOUNT,
        "导出备份", "从备份恢复", "导入数据", "导出数据", "本地备份", "换手机"),
    DEVICES("DevicesPage", "我的设备", HostDeepLink.TAB_ACCOUNT,
        "设备"),
    PRIVACY("PrivacyPage", "隐私", HostDeepLink.TAB_ACCOUNT,
        "本地优先", "联网功能", "本机数据", "剪贴板历史", "匿名使用统计", "用水杉账号翻译候选", "隐私模式",
        "隐私政策");

    private static final String PACKAGE = "app.msime.android.home.";

    private final String className;
    private final String title;
    private final int tab;
    private final List<String> keywords;

    PageId(String simpleName, String title, int tab, String... keywords) {
        this.className = PACKAGE + simpleName;
        this.title = title;
        this.tab = tab;
        this.keywords = ListPolicy.copyOrEmpty(Arrays.asList(keywords));
    }

    /** 页面 Fragment 的全限定类名，交给 `FragmentFactory.instantiate`。 */
    public String className() { return className; }

    /** 默认标题；需要参数决定标题的页面（例如词库详情）自己覆盖。 */
    public String title() { return title; }

    /** 所属 tab，取值是 {@link HostDeepLink} 的 `TAB_*`。 */
    public int tab() { return tab; }

    /** 设置首页搜索用的行标题，不含页面标题本身。 */
    public List<String> keywords() { return keywords; }

    /** 能不能从搜索直接打开：要参数才能打开的页面不能。 */
    public boolean searchable() { return !keywords.isEmpty(); }

    /** 标题或任一关键词包含查询（忽略大小写）时为真；空查询不匹配任何页面。 */
    public boolean matches(String query) {
        String needle = TextPolicy.lowercaseTrimmed(query);
        if (needle.isEmpty()) return false;
        if (TextPolicy.lowercase(title).contains(needle)) return true;
        for (String keyword : keywords) {
            if (TextPolicy.lowercase(keyword).contains(needle)) return true;
        }
        return false;
    }

    /** 按页面类名反查；DetailPage 用它取默认标题。不是注册过的页面时返回 null。 */
    @Nullable public static PageId forClassName(String className) {
        for (PageId page : values()) {
            if (page.className.equals(className)) return page;
        }
        return null;
    }

    /** 按深链给出的名字查；名字不是本枚举的成员时返回 null 而不是抛异常。 */
    @Nullable public static PageId fromName(@Nullable String name) {
        if (name == null) return null;
        try {
            return valueOf(name);
        } catch (IllegalArgumentException unknown) {
            return null;
        }
    }
}
