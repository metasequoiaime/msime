import app.msime.android.OfflineImportPolicy;
import java.util.List;

/** Offline import of a local voice model or the voice runtime: which picked documents are copied, how far a copy may read, how the files are listed. */
public final class OfflineImportPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        long[] expected = {158_000_000L, 4_096L};
        check(OfflineImportPolicy.shouldCopy(158_000_000L, expected) && OfflineImportPolicy.shouldCopy(4_096L, expected),
            "a document with a needed length is copied");
        check(!OfflineImportPolicy.shouldCopy(12L, expected), "a document no needed file can match is skipped");
        check(OfflineImportPolicy.shouldCopy(-1L, expected), "an unknown length is copied, bounded by copyLimit");
        check(!OfflineImportPolicy.shouldCopy(0L, new long[0]), "nothing is needed, nothing is copied");
        check(OfflineImportPolicy.copyLimit(expected) == 158_000_000L, "a copy stops past the largest needed file");
        check(OfflineImportPolicy.copyLimit(new long[0]) == 0L, "no files means no budget");
        check(OfflineImportPolicy.MAX_FILES == 16, "the shared install accepts at most 16 files");
        check(OfflineImportPolicy.fileLine("model.tar.bz2", 158_000_000L).equals("· model.tar.bz2（158.0 MB）"),
            "each file is listed with its size");
        String text = OfflineImportPolicy.instructions(List.of("· a（1.0 MB）", "· b（2.0 MB）"));
        check(text.endsWith("· a（1.0 MB）\n· b（2.0 MB）") && text.contains("改了文件名也没关系"),
            "the instructions end with the file list and say names do not matter");
        System.out.println("Android offline voice import: copy selection, limit and instructions passed");
    }
}
