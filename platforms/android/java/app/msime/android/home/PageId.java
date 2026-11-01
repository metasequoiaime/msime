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
 * <p>关键词取自对应设计图上的行标题，搜索时与标题一起匹配；{@link #LEXICON_DETAIL} 和 {@link #CLIPBOARD_EDIT} 需要参数才能打开，{@link #COMMUNITY} 要从功能页带着作品类型进去，所以都没有关键词，不出现在搜索结果里。页面里新加的开关、导航和滑块行要同时把行标题加进这里，否则设置首页搜不到它（#6132 的「浮动键盘」就是这样漏掉的）；`tests/home/SettingsSearchIndexSmoke.java` 逐页核对。
 */
public enum PageId {
    SKINS("SkinsPage", "皮肤", HostDeepLink.TAB_SETTINGS,
        "跟随系统", "水杉四季", "水杉", "浅色", "纸白", "夜青", "墨", "春芽", "夏荫", "秋杉", "冬雪", "AI 设计皮肤",
        "社区皮肤", "搜索皮肤", "皮肤分类"),
    AI_SKIN("AiSkinPage", "AI 设计皮肤", HostDeepLink.TAB_SETTINGS,
        "描述一句话生成", "配色", "按键音效", "按键动画", "生成皮肤"),
    KEYBOARD_OPTIONS("KeyboardOptionsPage", "键盘", HostDeepLink.TAB_SETTINGS,
        "布局", "英文自动大写", "中文键盘", "拼音显示位置", "输入框", "候选栏", "中英键轮换其他语言", "交换语言键与数字键", "五笔键盘", "双拼键位提示", "组字时保留 Shift", "辅助码", "键盘高度", "按键布局", "分离式键盘", "手机横屏分离式键盘", "分离启用宽度", "分离中间空隙", "竖屏也分离", "大屏单手时侧边显示数字键盘", "侧边数字键盘", "单手", "折叠屏", "内屏", "外屏",
        "浮动键盘", "物理键盘时候选栏跟随光标", "键盘底栏", "键盘底部高度", "底部高度", "底部间距", "底部留白", "键盘抬高", "符号分类顺序", "按键反馈", "按键音", "音效", "按键振动", "按键弹出预览", "按键动画", "手势",
        "滑动输入符号", "滑动方向", "下滑", "上滑", "九键滑动输入数字", "九键滑动距离", "快速删除范围", "滑行输入", "空格键滑动移动光标", "长按空格语音输入", "按钮顺序", "键盘工具栏", "显示方式", "候选栏大小", "工具栏尺寸", "表情", "常用语",
        "皮肤", "剪贴板", "输入方式", "文本编辑", "撤销", "重做", "语音", "随机数字", "文本编辑按键顺序", "按键间距", "行间距", "剪贴板排列", "剪贴板分类", "剪贴板删除数据验证", "撤销时长", "历史记录上限", "单列", "双列", "导出文件夹", "导出剪贴板", "工具栏显示最近复制", "最近复制", "复制后自动显示快捷键", "快捷键功能", "分词",
        "触控板", "优先使用触控板", "按住工具临时打开", "水平移动距离", "垂直移动距离", "双击选词与三击选行", "逐步振动", "显示移动轨迹", "光标文字放大镜"),
    /** 键盘页「按键布局」进来的子页：三类键面的底行样式、中/英键位置、九键与数字键盘的左侧符号和顶部数字行（{@link KeyLayoutStylesPage}）。「九键数字键盘左侧符号」是这一行原来在键盘页时的名字，留作别名。 */
    KEY_LAYOUT_STYLES("KeyLayoutStylesPage", "按键布局", HostDeepLink.TAB_SETTINGS,
        "26 键", "14 键与 18 键", "9 键", "九键", "底行", "标准", "逗号句号", "逗号句号在空格两侧", "长空格", "长空格布局", "表情键", "底栏带标点",
        "右侧大回车", "大回车", "顶部数字行", "数字行", "「符」与「123」互换", "符号键位置", "中/英键放在空格左边", "中英切换键", "数字键盘", "空格在右列", "空格在 0 右边", "空格在 0 左边",
        "九键左侧符号", "数字键盘左侧符号", "九键数字键盘左侧符号", "左侧符号", "符号栏"),
    TOOLBAR_EDITOR("ToolbarEditorPage", "编辑工具栏", HostDeepLink.TAB_SETTINGS, "编辑工具栏"),
    PLUGINS("PluginsPage", "插件", HostDeepLink.TAB_SETTINGS,
        "我的插件", "社区插件", "音效包", "指令表", "短语表", "辅助码表", "单词本", "符号集", "制作插件", "新建插件", "导入插件",
        "导入 .zip", "插件类型", "插件范围", "我的作品", "发布我的插件", "搜索插件"),
    /** 一个已安装插件的详情，参数 `kind`、`id` 选包；从插件页进来，本身不进设置搜索。 */
    PLUGIN_DETAIL("PluginDetailPage", "插件详情", HostDeepLink.TAB_SETTINGS),
    /** 插件编辑器，参数 `kind`、`id` 都给时编辑那个包；从插件页或插件详情进来。 */
    PLUGIN_EDITOR("PluginEditorPage", "新建插件", HostDeepLink.TAB_SETTINGS),
    /** 社区里的一个插件，参数 `id` 是发布的 UUID。 */
    COMMUNITY_PLUGIN("CommunityPluginPage", "社区插件", HostDeepLink.TAB_SETTINGS),
    PLUGIN_PUBLISH("PluginPublishPage", "发布插件", HostDeepLink.TAB_SETTINGS),
    CHAT("ChatFragment", "AI 对话", HostDeepLink.TAB_SETTINGS),
    AI_SETTINGS("AiSettingsPage", "AI 润色与回复", HostDeepLink.TAB_SETTINGS,
        "启用 AI 入口", "服务商", "端点 URL", "模型", "获取模型列表", "AI 接入说明与 API Key", "凭据", "润色提示词", "AI 对话", "社区回复模板", "浏览更多回复模板"),
    TYPING("TypingPage", "输入", HostDeepLink.TAB_SETTINGS,
        "语言与方案", "全拼", "双拼", "九键双拼方案与自定义", "五笔", "注音", "笔画", "手写", "粤拼", "普通话", "粤语", "英语", "日语", "韩语", "越南语", "藏语", "添加方案", "中文", "中文字符集", "简体", "繁体", "中英文之间加空格",
        "拼音纠错", "模糊音", "只出单字", "云候选", "云候选服务", "来源", "自定义接口设置", "接口地址", "请求方法", "请求头", "请求体模板", "响应格式", "JSON 路径", "超时", "缓存云候选", "日语九键连点切换", "辅助码", "启用辅助码", "辅助码方案", "辅助码模式", "在候选栏中显示双拼辅助码", "在候选栏中显示全拼辅助码", "全拼一码严格筛选", "双拼一码严格筛选", "翻译", "候选词翻译",
        "离线英文释义", "翻译目标语言", "五笔", "退格键回退已选候选词", "联系人", "电话本", "导入联系人"),
    FUZZY_PINYIN("FuzzyPinyinPage", "模糊音规则", HostDeepLink.TAB_SETTINGS,
        "启用模糊音", "平翘舌", "声母", "前后鼻音", "其他韵母", "z ↔ zh", "c ↔ ch", "s ↔ sh",
        "n ↔ l", "f ↔ h", "r ↔ l", "an ↔ ang", "en ↔ eng", "in ↔ ing", "ian ↔ iang", "uan ↔ uang"),
    CONTACTS("ContactsPage", "联系人", HostDeepLink.TAB_SETTINGS,
        "联系人", "电话本", "导入联系人", "清空联系人", "刷新联系人"),
    TRANSLATION_SERVICE("TranslationServicePage", "翻译服务", HostDeepLink.TAB_SETTINGS,
        "不使用", "水杉账号", "小牛翻译", "腾讯云机器翻译", "自定义", "DeepLX", "DeepL", "接口地址", "API Key", "App ID",
        "SecretId", "SecretKey", "地域", "测试连接", "AI 辅助", "当前服务商与模型"),
    WUBI_LOOKUP("WubiLookupPage", "五笔查码", HostDeepLink.TAB_SETTINGS,
        "查汉字的五笔编码", "五笔编码", "86 五笔", "98 五笔", "反查"),
    NINE_KEY_SHUANGPIN("NineKeyShuangpinPage", "九键双拼", HostDeepLink.TAB_SETTINGS,
        "方案", "使用九键双拼键盘", "米旮旯 3", "米旮旯 2", "辜氏 2 低重码", "自定义", "键位", "自定义表",
        "从内置方案开始", "导入", "导出", "保存并使用", "声母", "韵母", "零声母"),
    EXPRESSION("ExpressionPage", "表达", HostDeepLink.TAB_SETTINGS,
        "标点", "使用英文标点", "引号样式", "直角引号", "方头括号样式", "空心括号", "自动补全成对标点", "智能标点", "智能", "整句候选", "整句联想", "上屏后联想", "联想栏按键", "关闭联想", "下一词", "神经模型重排", "英文自动纠正", "英文联想",
        "连续多语言输入（实验）", "连续输入：", "中文", "英文", "日文",
        "候选混输", "中英混输", "触发字母数", "首页英文候选数", "候选带符号", "候选带 emoji", "候选带颜文字", "发现短语", "社区短语", "浏览更多短语"),
    LEXICON("LexiconPage", "词库", HostDeepLink.TAB_SETTINGS,
        "自动更新词库", "检查词库更新", "恢复上一版词库", "已安装", "拼音词库", "管理", "新建词库", "导入词库", "刷新词库", "导出词库", "发现词库", "社区词库", "浏览更多词库", "提交缺失词条", "记忆新词", "学习", "背单词", "云词库"),
    LEXICON_DETAIL("LexiconDetailPage", "词库详情", HostDeepLink.TAB_SETTINGS),
    VOCABULARY_REVIEW("VocabularyReviewPage", "背单词", HostDeepLink.TAB_SETTINGS,
        "今日待复习", "已完成", "未开始", "词书", "每日新词", "导入词表（CSV / TXT）", "删除这个词表", "不认识", "认识", "清空复习进度"),
    PHRASES("PhrasesPage", "常用语", HostDeepLink.TAB_SETTINGS,
        "添加常用语", "修改常用语", "删除常用语", "浏览社区短语包", "分类", "默认展开"),
    VOICE("VoicePage", "语音输入", HostDeepLink.TAB_SETTINGS,
        "识别服务", "本地模型", "本地识别准备", "已选模型", "本地语音运行库", "离线", "从文件导入", "从文件导入运行库", "导入", "鉴权方式", "App ID", "API Key", "密钥", "识别接口", "资源 ID", "模型", "接口地址",
        "豆包", "SiliconFlow", "OpenAI", "Groq", "EveryAPI", "Mistral", "百炼", "DeepSeek",
        "润色", "润色识别结果", "润色服务", "润色 API Key", "润色模型", "润色接口地址", "润色方案", "方案内容", "测试连接",
        "识别", "识别语言", "自动添加标点", "离线识别", "启动方式", "长按空格", "本地停顿时间", "本地识别灵敏度", "隐私", "上传语音以改进识别"),
    HANDWRITING("HandwritingPage", "手写输入", HostDeepLink.TAB_SETTINGS,
        "书写", "书写模式", "叠写", "识别等待时间", "识别后显示拼音", "笔迹", "笔迹颜色", "笔迹粗细"),
    DEVELOPER("DeveloperPage", "开发者选项", HostDeepLink.TAB_SETTINGS,
        "MCP 开发者访问", "上传日志", "上传日志供开发者通过 MCP 读取", "保留时长", "可访问的日志", "崩溃日志", "性能日志", "输入事件", "配置快照",
        "调试", "日志总开关", "显示调试信息", "记录输入日志", "日志级别", "数据", "默认导出文件夹", "导出诊断包", "分享诊断包", "重置所有设置"),
    FEEDBACK("FeedbackPage", "帮助与反馈", HostDeepLink.TAB_ACCOUNT,
        "反馈", "反馈类型", "附带诊断信息", "描述", "添加截图", "提交", "插入设备信息"),
    ABOUT("AboutPage", "关于", HostDeepLink.TAB_ACCOUNT,
        "版本", "检查更新", "更新", "自动更新", "更新通道", "稳定版", "设备信息", "复制设备信息", "官网", "法律信息",
        "用户协议", "隐私政策", "项目主页", "开源许可", "给我们评分", "在管理界面中查看",
        "开发者选项", "日志", "诊断包", "开发者访问"),
    HELP("HelpPage", "使用帮助", HostDeepLink.TAB_ACCOUNT,
        "启用键盘", "打开系统键盘设置", "打字", "选择候选词", "换一种输入方案", "换皮肤与布局", "遇到问题",
        "更新后仍有旧问题", "重装前先导出备份", "完整文档", "反馈问题与建议"),
    DOWNLOAD("DownloadPage", "其他平台下载", HostDeepLink.TAB_ACCOUNT,
        "在电脑上打开", "复制链接", "电脑", "发送链接", "Windows", "macOS", "Linux", "HarmonyOS", "手机和平板",
        "iOS", "iPadOS", "Android"),
    CLOUD_CLIPBOARD("CloudClipboardPage", "云剪贴板", HostDeepLink.TAB_ACCOUNT,
        "保留时长", "最近", "清空", "云剪贴板同步方式", "同步方式", "自建中继", "中继地址", "中继令牌",
        "WebDAV 网盘", "坚果云", "服务器地址", "网盘账号", "应用密码", "测试连接"),
    DICTIONARY_SYNC("DictionarySyncPage", "词库同步", HostDeepLink.TAB_ACCOUNT,
        "我的词条", "WebDAV 网盘", "坚果云", "Nextcloud", "设置 WebDAV 网盘", "服务器地址", "账号", "应用密码", "测试连接",
        "停止词库同步", "立即同步", "共享的词", "待同步", "被挡下的删除", "同步删除", "从网盘恢复", "等待允许", "核对码",
        "设备", "重置密钥", "以本机为准重新开始"),
    CLIPBOARD_CATEGORIES("ClipboardCategoriesPage", "剪贴板分类", HostDeepLink.TAB_SETTINGS,
        "手动分类", "分类名称", "添加分类", "修改分类名称", "删除分类", "重新读取"),
    CLIPBOARD_EDIT("ClipboardEditPage", "编辑剪贴板记录", HostDeepLink.TAB_SETTINGS),
    CLIPBOARD_SEARCH("ClipboardSearchPage", "剪贴板历史", HostDeepLink.TAB_SETTINGS,
        "搜索剪贴板", "剪贴板记录", "复制过的文字"),
    PROFILE("ProfilePage", "个人资料", HostDeepLink.TAB_ACCOUNT,
        "账号", "昵称", "水杉 ID", "邮箱", "登录方式", "关联", "云端数据", "导出我的数据", "退出登录", "注销账号"),
    BACKUP("BackupPage", "备份与恢复", HostDeepLink.TAB_ACCOUNT,
        "导出备份", "从备份恢复", "导入数据", "导出数据", "本地备份", "换手机"),
    DEVICES("DevicesPage", "我的设备", HostDeepLink.TAB_ACCOUNT,
        "设备"),
    QQ_LINKS("QqLinksPage", "绑定 QQ 群助手", HostDeepLink.TAB_ACCOUNT,
        "QQ", "绑定码", "解绑", "/stats", "群助手"),
    PRIVACY("PrivacyPage", "隐私", HostDeepLink.TAB_ACCOUNT,
        "本地优先", "联网功能", "本机数据", "剪贴板历史", "匿名使用统计", "用水杉账号翻译候选", "隐私模式",
        "截屏与录屏", "防止截屏", "隐私政策"),
    /** 隐私页「社区」组进来的子页（{@link BlockedAuthorsPage}）：登录账号屏蔽的社区作者和取消屏蔽。 */
    BLOCKED_AUTHORS("BlockedAuthorsPage", "已屏蔽的作者", HostDeepLink.TAB_ACCOUNT,
        "屏蔽作者", "取消屏蔽"),
    /** 一类社区作品的完整列表，参数 `kind` 选类型（见 {@link CommunityFragment#ARG_KIND}）；从各功能页的「浏览更多…」进来，本身不进设置搜索，搜索里出现的是那几个功能页。皮肤和词库不压栈，{@link SettingsNavigator} 把它们切到社区 tab 的对应分类。 */
    COMMUNITY("CommunityFragment", "社区", HostDeepLink.TAB_SETTINGS);

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
