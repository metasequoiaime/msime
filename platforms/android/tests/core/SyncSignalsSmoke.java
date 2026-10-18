import app.msime.android.SyncSwitch;

public final class SyncSignalsSmoke {
    public static void main(String[] arguments) throws Exception {
        // 分类名是 Rust 的 mark_dirty 认的那四个；记改动、看开关都在 Rust 里，由设备冒烟对着真的 libmsime_android.so 核对。
        check(SyncSwitch.SECTIONS.equals(java.util.List.of("settings", "phrases", "skins", "dictionary")),
            "settings, phrases, skins and dictionary");
        for (String section : SyncSwitch.SECTIONS) check(SyncSwitch.validSection(section), section);
        check(!SyncSwitch.validSection("tokens"), "an unknown section is refused");
        check(!SyncSwitch.validSection(null), "a missing section is refused");
        check(SyncSwitch.validLoginKind("google") && SyncSwitch.validLoginKind("apple")
            && SyncSwitch.validLoginKind("email"), "the three real-account sign-ins");
        check(!SyncSwitch.validLoginKind("anonymous"), "the anonymous account never syncs");
        System.out.println("Android sync signals passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
