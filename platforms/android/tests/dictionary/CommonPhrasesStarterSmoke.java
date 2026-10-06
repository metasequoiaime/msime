package app.msime.android;

import java.io.File;
import java.nio.file.Files;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/** 预置示例常用语：内容本身的约束，以及记录「哪些示例还没被用户认领」的标记文件读改写。 */
public final class CommonPhrasesStarterSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        List<String> starters = CommonPhrasesStore.STARTER_PHRASES;
        check(starters.size() == 8, "eight starter phrases");
        check(new HashSet<>(starters).size() == starters.size(), "starter phrases are distinct");
        for (String text : starters) {
            check(CommonPhrasesStore.validText(text), "every starter is a valid phrase: " + text);
            check(text.indexOf('\n') < 0, "the marker stores one starter per line");
            check(text.indexOf('@') < 0, "no starter carries an email address: " + text);
        }

        Set<String> sample = new LinkedHashSet<>(List.of("马上到", "好的，收到"));
        check(CommonPhrasesStore.decodeStarters(CommonPhrasesStore.encodeStarters(sample)).equals(sample),
            "the marker round-trips");
        check(CommonPhrasesStore.decodeStarters(new byte[0]).isEmpty(), "an empty marker from an older build records nothing");

        File directory = Files.createTempDirectory("msime-starters").toFile();
        File marker = new File(directory, CommonPhrasesStore.STARTER_MARKER);
        try {
            check(CommonPhrasesStore.editStarters(marker, false, current -> Set.of("x")).isEmpty(),
                "without a marker nothing is recorded");
            check(!marker.exists(), "reading or adopting never creates the marker, which would stop seeding");

            Set<String> seeded = CommonPhrasesStore.editStarters(marker, true, current -> Set.copyOf(starters));
            check(marker.isFile() && seeded.equals(Set.copyOf(starters)), "seeding records what was added");

            // 另一个进程同时第一次读，它的添加全被当成重复拒收，只按并集写入空集合：已有的记录不能被抹掉。
            Set<String> union = CommonPhrasesStore.editStarters(marker, true, current -> {
                Set<String> next = new LinkedHashSet<>(current);
                next.addAll(Set.of());
                return next;
            });
            check(union.equals(seeded), "a second seeder keeps the first one's record");

            Set<String> adopted = CommonPhrasesStore.editStarters(marker, false, current -> {
                Set<String> next = new LinkedHashSet<>(current);
                next.remove("马上到");
                return next;
            });
            check(!adopted.contains("马上到") && adopted.size() == starters.size() - 1, "adopting removes only that text");
            check(CommonPhrasesStore.editStarters(marker, false, current -> current).equals(adopted),
                "the adoption is persisted");
        } finally {
            Files.deleteIfExists(marker.toPath());
            Files.deleteIfExists(directory.toPath());
        }
        System.out.println("Android common phrase starters passed");
    }
}
