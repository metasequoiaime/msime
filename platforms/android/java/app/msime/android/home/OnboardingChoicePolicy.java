package app.msime.android.home;

import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;

/**
 * 引导页「选方案」「显示译文」两步在偏好还读不到时怎么显示、读到后要写什么。
 *
 * <p>首次安装时偏好存储的目录由词库准备（`Bootstrap.prepare` → `prepare_host`）定下，准备完成前 `HostStore.loadPreferences` 只能返回 null。引导页不让用户等它：点下的方案、拨动的开关先记成待保存的选择，界面立即按它显示，偏好可读后再由 {@link OnboardingChoices} 写进去。这里是其中不碰 Android 和 JSON 的判断，check-host 直接编译并在 JVM 里测它。
 */
public final class OnboardingChoicePolicy {
    private OnboardingChoicePolicy() {}

    /** 方案页和译文页脚注该说的那一种情况。 */
    public enum Footnote {
        /** 偏好可读，也没有待保存的选择：方案页不写脚注，译文页写它固定的说明。 */
        NONE,
        /** 偏好可读，待保存的选择正在写入。 */
        SAVING,
        /** 第一次读取还没回来。 */
        READING,
        /** 词库准备失败，偏好读不到；点脚注重试。 */
        FAILED,
        /** 词库还在准备，用户已经选好，准备好后自动保存。 */
        CHOSEN_WAITING,
        /** 词库还在准备，用户还没选。 */
        WAITING
    }

    /**
     * 脚注的情况。准备失败优先于「正在读取」：失败时第一次读取必然也读不到，先说失败才能让用户去重试。
     *
     * @param readable 偏好已经读到
     * @param loaded 第一次读取已经回来（无论读没读到）
     * @param failed 词库准备处于失败状态
     * @param pending 有待保存的选择
     */
    public static Footnote footnote(boolean readable, boolean loaded, boolean failed, boolean pending) {
        if (readable) return pending ? Footnote.SAVING : Footnote.NONE;
        if (failed) return Footnote.FAILED;
        if (!loaded) return Footnote.READING;
        return pending ? Footnote.CHOSEN_WAITING : Footnote.WAITING;
    }

    /** 待保存的方案 id 对应的入口；不认识的 id 或本版本不提供的方案当作没有选择。 */
    public static KeyboardScheme pendingScheme(String pendingId, AppEdition edition) {
        KeyboardScheme scheme = KeyboardScheme.fromPreferenceId(pendingId);
        return scheme != null && scheme.offeredBy(edition) ? scheme : null;
    }

    /** 卡片上显示为已选的方案：有待保存的选择时按它，否则按偏好里存的；两者都没有时为 null，一张都不选。 */
    public static KeyboardScheme displayedScheme(KeyboardScheme pending, KeyboardScheme stored) {
        return pending != null ? pending : stored;
    }

    /**
     * 「双拼」卡片代表的那个双拼方案：待保存的选择是双拼时用它（用户点的就是这张卡片当时代表的方案），否则沿用偏好里已选的双拼方案，都不是双拼时是设计写的「默认小鹤」。
     */
    public static KeyboardScheme shuangpinCard(KeyboardScheme pending, KeyboardScheme stored) {
        if (pending != null && pending.shuangpinProfile() != null) return pending;
        if (stored != null && stored.shuangpinProfile() != null) return stored;
        return KeyboardScheme.XIAOHE;
    }

    /**
     * 偏好读到后要写的方案，null 表示不用写。
     *
     * <p>与偏好里已有的相同时不写。待保存的是双拼、偏好里已经是另一种双拼时也不写：偏好没读到时「双拼」卡片只能代表小鹤，用户点它的意思是「用双拼」，不是把已选的自然码、微软等改回小鹤，这与偏好可读时卡片沿用已选双拼方案的规则一致。本版本不提供的方案不写。
     */
    public static KeyboardScheme schemeToWrite(String pendingId, KeyboardScheme stored, AppEdition edition) {
        KeyboardScheme pending = pendingScheme(pendingId, edition);
        if (pending == null || pending == stored) return null;
        if (pending.shuangpinProfile() != null && stored != null && stored.shuangpinProfile() != null) return null;
        return pending;
    }

    /** 偏好读到后要写的「显示译文」值，null 表示没有待保存的值或与已存的相同。 */
    public static Boolean glossToWrite(Boolean pending, boolean stored) {
        if (pending == null || pending == stored) return null;
        return pending;
    }
}
