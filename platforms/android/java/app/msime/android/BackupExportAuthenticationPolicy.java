package app.msime.android;

/** 导出备份包含输入记录，开始导出前必须通过系统设备认证。 */
public final class BackupExportAuthenticationPolicy {
    private BackupExportAuthenticationPolicy() {}

    public static boolean requiresAuthentication() { return true; }
}
