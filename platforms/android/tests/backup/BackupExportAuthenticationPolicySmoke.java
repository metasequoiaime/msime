package app.msime.android;

/** #9610：每次点击导出备份都必须先通过系统认证。 */
public final class BackupExportAuthenticationPolicySmoke {
    public static void main(String[] args) {
        if (!BackupExportAuthenticationPolicy.requiresAuthentication()) {
            throw new AssertionError("backup export must require device authentication");
        }
        System.out.println("BackupExportAuthenticationPolicySmoke ok");
    }
}
