package app.msime.android;

/**
 * 引擎会话还在建时，工具栏的表情、常用语、皮肤、输入方式和底行「中」键能不能点、点了做什么（#10086）。
 *
 * <p>每换一个输入框宿主都会销毁会话、在工作线程上同步个人词库、读实时偏好，再回主线程建新会话，这段空档在低端机上有一两秒。表情和常用语只读资源目录与偏好目录，「中」键只改宿主的中英状态，它们不需要会话，空档里照常可用。皮肤和输入方式的保存要经会话应用偏好，空档里点按先记下，会话和偏好就绪后立即打开面板。没有会话、也没有会话在建的输入框（密码框、数字框、启动失败退回直接输入）保持原样：这些入口不可用，「中」键只留长按切换输入法。
 */
public final class ToolbarReadinessPolicy {
    private ToolbarReadinessPolicy() {}

    /** 当前输入框的引擎会话状态。 */
    public enum Session {
        /** 会话已建好。 */
        READY,
        /** 会话还没建好，但这个输入框的启动已经排上、没有被换代或取消。 */
        STARTING,
        /** 没有会话，也没有会话在建。 */
        UNAVAILABLE
    }

    /** 点按皮肤或输入方式时做什么。 */
    public enum PanelTap {
        /** 立即打开面板。 */
        OPEN,
        /** 记下这一下，会话和偏好就绪后打开。 */
        DEFER,
        /** 忽略。 */
        IGNORE
    }

    /** 点按「中」键时做什么。 */
    public enum LanguageTap {
        /** 照常交给引擎（中英切换或轮换到其他语言）。 */
        ENGINE,
        /** 会话还在建：只翻转宿主的中英状态并记住，会话建好时按它交给引擎。 */
        HOST_TOGGLE,
        /** 会话还在建，而这一下要轮换到其他语言键盘：记下，会话和偏好就绪后再按一次。 */
        DEFER,
        /** 忽略。 */
        IGNORE
    }

    public static Session session(boolean sessionReady, boolean startupPending) {
        if (sessionReady) return Session.READY;
        return startupPending ? Session.STARTING : Session.UNAVAILABLE;
    }

    /** 表情面板只用资源目录里的表情目录和输入连接。 */
    public static boolean emojiEnabled(Session session, boolean connection, boolean resourcesKnown) {
        return session != Session.UNAVAILABLE && connection && resourcesKnown;
    }

    /** 常用语面板只读偏好目录里的常用语文档。 */
    public static boolean phraseEnabled(Session session, boolean preferencesDirectoryKnown) {
        return session != Session.UNAVAILABLE && preferencesDirectoryKnown;
    }

    /** 皮肤、输入方式按钮是否可点：会话在建时也可点（点按推迟），会话建好后要等偏好读到。 */
    public static boolean preferencePanelEnabled(Session session, boolean preferencesLoaded) {
        return preferencePanelTap(session, preferencesLoaded) != PanelTap.IGNORE;
    }

    public static PanelTap preferencePanelTap(Session session, boolean preferencesLoaded) {
        if (session == Session.STARTING) return PanelTap.DEFER;
        return session == Session.READY && preferencesLoaded ? PanelTap.OPEN : PanelTap.IGNORE;
    }

    /** 推迟的那一下现在能不能执行：会话和偏好都要就绪。面板类的那一下在用户已经打开了别的面板时作废，由调用方判断。 */
    public static boolean deferredTapReady(Session session, boolean preferencesLoaded) {
        return session == Session.READY && preferencesLoaded;
    }

    /** 「中」键是否按可用画（不画淡、读屏不念「暂不可用」）。 */
    public static boolean languageKeyActive(Session session) {
        return session != Session.UNAVAILABLE;
    }

    /**
     * @param cycles 设置「中英键轮换其他语言」是否打开
     * @param english 英文直输是否开着
     * @param otherLanguageSelected 当前方案是不是其他语言键盘
     */
    public static LanguageTap languageTap(Session session, boolean cycles, boolean english,
            boolean otherLanguageSelected) {
        if (session == Session.READY) return LanguageTap.ENGINE;
        if (session == Session.UNAVAILABLE) return LanguageTap.IGNORE;
        // 轮换打开时，从英文或其他语言出发的下一步是切方案，要会话和偏好算出启用了哪些语言键盘，只能推迟；从中文出发的下一步总是进英文，和轮换关着时一样只翻中英。
        if (cycles && (english || otherLanguageSelected)) return LanguageTap.DEFER;
        return LanguageTap.HOST_TOGGLE;
    }
}
