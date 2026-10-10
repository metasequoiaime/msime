import app.msime.android.ReadingRowPolicy;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

public final class ReadingRowPolicySmoke {
    static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
    }

    private static final String PREFERENCES_SOURCE = "crates/client-core/src/preferences.rs";

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(PREFERENCES_SOURCE))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + PREFERENCES_SOURCE + " not found above user.dir");
    }

    public static void main(String[] args) throws IOException {
        // #6107：读音字号按「预编辑字号」相对共享默认值缩放。没改过设置（共享默认 15）的用户读音仍是设计的 12sp，调大调小都跟着变。
        check(ReadingRowPolicy.textSizeSp(ReadingRowPolicy.DEFAULT_PREEDIT_FONT_SIZE) == ReadingRowPolicy.DESIGN_TEXT_SP,
            "the shared default keeps the 12sp reading");
        check(ReadingRowPolicy.DESIGN_TEXT_SP == 12f, "the design reading size is 12sp");
        check(Math.abs(ReadingRowPolicy.textSizeSp(12) - 9.6f) < 1e-4f, "the smallest setting shrinks the reading");
        check(Math.abs(ReadingRowPolicy.textSizeSp(20) - 16f) < 1e-4f, "a larger setting grows the reading");
        check(Math.abs(ReadingRowPolicy.textSizeSp(32) - 25.6f) < 1e-4f, "the largest setting grows the reading");
        check(ReadingRowPolicy.textSizeSp(16) > ReadingRowPolicy.textSizeSp(15), "every step changes the size");
        // 共享默认值改了，这里的换算基准也得跟着改，否则没改过设置的用户读音会悄悄变大或变小。
        String preferences = Files.readString(repoRoot().resolve(PREFERENCES_SOURCE));
        check(preferences.replaceAll("\\s+", " ").contains("fn default_candidate_preedit_font_size() -> u8 { "
                + ReadingRowPolicy.DEFAULT_PREEDIT_FONT_SIZE + " }"),
            "DEFAULT_PREEDIT_FONT_SIZE matches client-core's default_candidate_preedit_font_size");

        // 3x 密度的设计高度 14dp = 42px。12sp 的读音在系统字体放大 1.15 倍时是 41.4px，默认字体 ascent 约 -39、descent 约 11，文字要 50px；按设计高度画就切掉 y、g 的下伸部（#5591）。厂商字体更高时差得更多。
        check(ReadingRowPolicy.heightPx(42, -39, 11, 0) == 50, "a scaled-up 12sp reading grows the row to its full ink height");
        // 读音视图自己的上下内边距也算进去。
        check(ReadingRowPolicy.heightPx(42, -39, 11, 4) == 54, "vertical padding is part of the row");
        // 系统字体不放大时 12sp 的读音（3x 上 ascent 约 -33、descent 约 9）本来就放得下，行高保持设计值，键盘总高与改动前相同。
        check(ReadingRowPolicy.heightPx(42, -33, 9, 0) == 42, "a reading that fits keeps the design height");
        // #6107：读音字号跟设置走（这里是 3x 上 24sp，ascent 约 -67、descent 约 18），再加读音和候选行之间 3dp（9px）的间距，行高随之变高，间距不被读音占掉。
        check(ReadingRowPolicy.heightPx(42, -67, 18, 9) == 94, "a large reading font plus the gap grows the row past the design height");
        // 默认 12sp 刚好填满设计高度时，加上间距后读音行比设计高出这段间距。
        check(ReadingRowPolicy.heightPx(42, -33, 9, 9) == 51, "the gap below the reading is added even when the reading fits");
        // 字体度量异常（descent 小于 ascent）或负的内边距不会把行高算成负数或比设计还矮。
        check(ReadingRowPolicy.heightPx(42, 10, -10, -6) == 42, "bad metrics never shrink the row");
        check(ReadingRowPolicy.heightPx(-1, 0, 0, 0) == 0, "a negative design height is not a height");
        System.out.println("Android reading row: the row is as tall as the reading's ink passed");
    }
}
