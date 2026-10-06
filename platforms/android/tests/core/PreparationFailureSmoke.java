import app.msime.android.PreparationFailure;

public final class PreparationFailureSmoke {
    static void check(boolean condition, String label) { if (!condition) throw new AssertionError(label); }

    public static void main(String[] args) {
        // Bootstrap 自己加的前缀去掉，只留共享层给的原因。
        check(PreparationFailure.describe(new IllegalStateException(
            "Shared resource verification/preparation failed: Runtime dictionary staging directory already exists"))
            .equals("Runtime dictionary staging directory already exists"), "bootstrap prefix");
        // 其他异常带上类名，绝对路径不出现在界面上。
        String io = PreparationFailure.describe(new java.io.IOException(
            "/data/user/0/app.msime.android/files/bootstrap/resources/msime-pinyin.db: Permission denied"));
        check(io.equals("IOException: …: Permission denied"), "path removed: " + io);
        // 空间不足说成用户能照着做的中文。
        check(PreparationFailure.describe(new java.io.IOException("No space left on device"))
            .startsWith("手机存储空间不足"), "storage full");
        check(PreparationFailure.describe(new UnsatisfiedLinkError()).equals("UnsatisfiedLinkError"), "no message");
        check(PreparationFailure.describe(new IllegalStateException("  ")).equals("IllegalStateException"), "blank message");
        check(PreparationFailure.describe(null).isEmpty(), "no error");
        String long_ = PreparationFailure.describe(new IllegalStateException("x".repeat(500)));
        check(long_.length() == 160 && long_.endsWith("…"), "bounded");
        check(PreparationFailure.describe(new IllegalStateException("a\n  b")).equals("IllegalStateException: a b"), "one line");
        System.out.println("Android first-run preparation failure: prefix, paths, bounds and fallbacks passed");
    }
}
