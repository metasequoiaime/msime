import app.msime.android.policy.InstallNoticePolicy;

public final class InstallNoticePolicySmoke {
    public static void main(String[] arguments) {
        check(InstallNoticePolicy.shouldShow("100", null), "首次安装显示说明");
        check(!InstallNoticePolicy.shouldShow("100", "100"), "同一次安装不重复显示");
        check(InstallNoticePolicy.shouldShow("200", "100"), "升级后再次显示说明");
        check(!InstallNoticePolicy.shouldShow("", "100"), "无效安装标记不显示");
        check(InstallNoticePolicy.dismissDelayMillis(0) == 4000, "短文案使用最短倒计时");
        check(InstallNoticePolicy.dismissDelayMillis(1000) == 30000, "长文案使用上限倒计时");
        check(InstallNoticePolicy.dismissDelayMillis(200) > InstallNoticePolicy.dismissDelayMillis(20),
            "倒计时随文案长度增加");
        System.out.println("Android install notice policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
