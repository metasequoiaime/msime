package app.msime.android;

/**
 * 「剪贴板删除数据验证」（#10408）：键盘剪贴板面板里删除本机记录（长按操作行的「删除」、左滑垃圾桶、多选删除）前要不要先过系统认证，以及开关本身怎么切换。
 *
 * <p>开关默认关闭，存在只属于本机的 {@link AndroidLocalSettings#CLIPBOARD_DELETE_AUTHENTICATION}，不同步也不进备份。打开和关闭都要先认证，第一次打开也不例外，这是它和「防止截屏」（{@link ScreenCaptureProtectionPolicy}）唯一的区别：那边第一次打开直接保存。设备没有可用的系统认证时一律拒绝切换，打开以后锁屏被移除的情况也一样，只有重新设锁屏才能关掉，与「防止截屏」的先例相同。
 *
 * <p>访客剪贴板不看开关，删除一律要认证（报告者给出、维护者接受的规则）；没有锁屏凭据的设备上访客模式因此删不掉记录，只提示先设锁屏。撤销删除不认证：撤销只把刚删掉的快照写回原来的存储。
 */
public final class ClipboardDeleteAuthenticationPolicy {
    private ClipboardDeleteAuthenticationPolicy() {}

    /** 系统认证不可用时给用户的提示：设置页拒绝切换和键盘里拒绝删除都用这一句。 */
    public static final String UNAVAILABLE_MESSAGE = "请先在系统设置中开启锁屏验证（指纹、图案、密码等）后再使用";

    /** 一次切换该怎么处理。 */
    public enum Decision {
        /** 下一值与当前值相同，什么也不做。 */
        UNCHANGED,
        /** 先通过系统认证再保存。 */
        AUTHENTICATE,
        /** 设备没有可用的系统认证，拒绝切换并保持原值。 */
        UNAVAILABLE,
    }

    /**
     * @param enabled 当前是否开着
     * @param next 用户要切换到的值
     * @param authenticationAvailable 系统认证此刻是否可用（`BiometricManager.canAuthenticate` 返回成功）
     */
    public static Decision decide(boolean enabled, boolean next, boolean authenticationAvailable) {
        if (next == enabled) return Decision.UNCHANGED;
        return authenticationAvailable ? Decision.AUTHENTICATE : Decision.UNAVAILABLE;
    }

    /** 删除一条或一批记录前是否要先认证：开关开着，或者正在用访客剪贴板。 */
    public static boolean requiresAuthentication(boolean enabled, boolean visitorMode) {
        return enabled || visitorMode;
    }

    /** 切换没成功时设置页给的提示；`next` 是用户想切到的值。 */
    public static String refusal(Decision decision, boolean next) {
        if (decision == Decision.UNAVAILABLE) return UNAVAILABLE_MESSAGE;
        return next ? "未通过系统认证，剪贴板删除数据验证保持关闭" : "未通过系统认证，剪贴板删除数据验证保持开启";
    }
}
