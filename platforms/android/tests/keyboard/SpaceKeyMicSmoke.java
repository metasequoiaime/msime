package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * 空格键上的麦克风跟着「长按空格语音输入」走（#10143）：关掉后长按空格不进语音，键面也不能再画麦克风。
 *
 * <p>{@link SpaceKeyFace} 是 View，JVM 冒烟里只有抛 `Stub!` 的桩，画不出来；这里测纯函数 {@link SpaceKeyFace#numberPadLabel}，再钉住 {@link ImeBottomRow} 给主空格和分离键盘右半边的空格都按同一个开关设置麦克风，以及语音页把「无」的意思写在启动方式上。
 */
public final class SpaceKeyMicSmoke {
    public static void main(String[] args) throws Exception {
        check(SpaceKeyFace.numberPadLabel(true).isEmpty(), "number pad space shows only the mic");
        check("空格".equals(SpaceKeyFace.numberPadLabel(false)), "number pad space without the mic is not blank");

        String bottomRow = read("core/ImeBottomRow.java");
        check(bottomRow.contains("private boolean spaceShowsMic() {\n        return touchPreference(AndroidLocalSettings.SPACE_VOICE);"),
            "the space mic follows the long-press voice switch");
        String update = body(bottomRow, "void updateActionRow() {");
        check(update.contains("face.setShowsMic(mic);")
                && update.contains("face.setSchemeLabel(numberPad ? SpaceKeyFace.numberPadLabel(mic) : spaceLabel());"),
            "the main space key sets its mic on every action-row refresh");
        check(body(bottomRow, "void syncSplitSpace() {").contains("splitSpace.setShowsMic(spaceShowsMic());"),
            "the split keyboard's right-hand space mirrors the mic");

        String voicePage = read("home/VoicePage.java");
        check(voicePage.contains("new OptionSheet(requireContext(), \"启动方式\", TRIGGER_SHEET_HINT)"),
            "the trigger sheet explains 无");
        check(voicePage.contains("recognition.nav(\"启动方式\", trigger == TRIGGER_NONE ? TRIGGER_NONE_HINT : null,"),
            "the trigger row explains 无 once chosen");
        check(voicePage.contains("\"键盘上不显示语音入口，长按空格只输入空格\""), "the 无 hint text");
        System.out.println("SpaceKeyMicSmoke passed");
    }

    /** `signature` 起到下一个同缩进的方法为止的一段源码。 */
    private static String body(String source, String signature) {
        int start = source.indexOf(signature);
        check(start >= 0, "missing " + signature);
        int end = source.indexOf("\n    }\n", start);
        check(end > start, "unterminated " + signature);
        return source.substring(start, end);
    }

    private static String read(String relative) throws Exception {
        return Files.readString(repoRoot().resolve("platforms/android/java/app/msime/android/" + relative),
            StandardCharsets.UTF_8);
    }

    private static Path repoRoot() {
        Path path = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (path != null) {
            if (Files.isRegularFile(path.resolve("platforms/android/java/app/msime/android/core/ImeBottomRow.java"))) return path;
            path = path.getParent();
        }
        throw new IllegalStateException("repository root not found");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
