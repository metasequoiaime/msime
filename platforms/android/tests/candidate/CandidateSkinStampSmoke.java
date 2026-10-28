import app.msime.android.CandidateSkinStamp;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.FileTime;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

/** 偏好轮询跳过整套重读的判据：偏好文件与候选包的元数据戳在文件没变时相同，原地改写、原子替换、装包、加图时都会变。 */
public final class CandidateSkinStampSmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("msime-skin-stamp");
        try {
            run(directory);
        } finally {
            try (Stream<Path> paths = Files.walk(directory)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
        System.out.println("Android candidate skin stamp: unchanged files keep the stamp, rewrites, replacements and installs change it");
    }

    private static void run(Path directory) throws Exception {
        Path preferences = directory.resolve("preferences.json");
        check(CandidateSkinStamp.file(preferences) == null, "a missing preferences file has no stamp");
        write(preferences, "{\"revision\":1}", 1_000);
        Object stamp = CandidateSkinStamp.file(preferences);
        check(stamp != null && stamp.equals(CandidateSkinStamp.file(preferences)), "an unchanged file keeps its stamp");
        write(preferences, "{\"revision\":2}", 2_000);
        check(!stamp.equals(CandidateSkinStamp.file(preferences)), "an in-place rewrite changes the stamp");
        stamp = CandidateSkinStamp.file(preferences);
        // 设置页保存是写临时文件再原子改名：同样大小、同样修改时间也换了 inode。
        Path pending = directory.resolve("preferences.json.pending");
        write(pending, "{\"revision\":3}", 2_000);
        Files.move(pending, preferences, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        check(!stamp.equals(CandidateSkinStamp.file(preferences)), "an atomic replacement changes the stamp");
        check(CandidateSkinStamp.file(directory) == null, "a directory is not a preferences file");

        Path skins = directory.resolve("skins");
        List<String> ids = List.of("paper", "night");
        Object none = CandidateSkinStamp.packages(skins, ids);
        check(none != null && none.equals(CandidateSkinStamp.packages(skins, ids)), "missing packages still stamp");
        Path paper = skins.resolve("paper");
        Files.createDirectories(paper.resolve("images"));
        write(paper.resolve("skin.toml"), "name = 'paper'", 3_000);
        write(paper.resolve("images").resolve("panel.png"), "AAAA", 3_000);
        Object installed = CandidateSkinStamp.packages(skins, ids);
        check(installed != null && !installed.equals(none), "installing a named package changes the stamp");
        check(installed.equals(CandidateSkinStamp.packages(skins, ids)), "an unchanged package keeps its stamp");
        // 只换图片、不动清单：资源更新不要求同时修改偏好或清单。
        write(paper.resolve("images").resolve("panel.png"), "BBBB", 4_000);
        Object replaced = CandidateSkinStamp.packages(skins, ids);
        check(!installed.equals(replaced), "rewriting a nested image changes the stamp");
        write(paper.resolve("images").resolve("badge.png"), "CC", 4_000);
        check(!replaced.equals(CandidateSkinStamp.packages(skins, ids)), "adding a resource changes the stamp");
        Files.setLastModifiedTime(skins, FileTime.fromMillis(6_000));
        Object current = CandidateSkinStamp.packages(skins, ids);
        Files.createDirectories(skins.resolve("other"));
        check(!current.equals(CandidateSkinStamp.packages(skins, ids)), "a package added beside the named ones changes the root stamp");
        check(CandidateSkinStamp.packages(skins, List.of("../paper", "")) != null, "unsafe ids are recorded, not resolved");
        Path crowded = skins.resolve("crowded");
        Files.createDirectories(crowded);
        for (int i = 0; i < 300; i++) write(crowded.resolve("frame" + i + ".png"), "x", 5_000);
        check(CandidateSkinStamp.packages(skins, List.of("crowded")) == null, "an oversized package has no stamp and is always read");
    }

    private static void write(Path file, String text, long modified) throws Exception {
        Files.write(file, text.getBytes(StandardCharsets.UTF_8));
        Files.setLastModifiedTime(file, FileTime.fromMillis(modified));
    }
}
