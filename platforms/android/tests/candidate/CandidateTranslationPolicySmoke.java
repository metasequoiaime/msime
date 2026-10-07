import app.msime.android.CandidateTranslationPolicy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class CandidateTranslationPolicySmoke {
    public static void main(String[] args) throws Exception {
        check(CandidateTranslationPolicy.targets("ja", "de").equals(List.of("ja", "de")),
            "primary and secondary languages preserved");
        check(CandidateTranslationPolicy.targets("invalid", "ja").equals(List.of("en", "ja")),
            "invalid primary falls back to English");
        check(CandidateTranslationPolicy.targets("en", "en").equals(List.of("en")),
            "duplicate secondary is not requested twice");
        check(CandidateTranslationPolicy.targets("en", null).equals(List.of("en")),
            "missing secondary preserves legacy behavior");
        check(CandidateTranslationPolicy.joinGlosses(List.of("hello", "こんにちは"))
                .equals("hello\nこんにちは"), "two glosses stay on separate rows");
        check(CandidateTranslationPolicy.insertionGlosses(" hello \nこんにちは\nhello")
                .equals(List.of("hello", "こんにちは")),
            "long press keeps distinct bounded gloss rows");
        check(CandidateTranslationPolicy.insertionGlosses("safe\n" + "x".repeat(4097))
                .equals(List.of("safe")), "oversized gloss rows are not insertable");
        check(CandidateTranslationPolicy.insertionGlosses("坏\uD800\n好")
                .equals(List.of("好")), "invalid Unicode gloss rows are not insertable");
        check(CandidateTranslationPolicy.glossLines(List.of("en"), true, false, Set.of()) == 1,
            "offline English keeps its row when online translation is off");
        check(CandidateTranslationPolicy.glossLines(List.of("ja"), true, false, Set.of()) == 0,
            "non-English has no row without online translation");
        check(CandidateTranslationPolicy.glossLines(List.of("en", "ja"), true, false, Set.of()) == 1,
            "only offline-capable targets reserve rows");
        check(CandidateTranslationPolicy.glossLines(List.of("en", "ja"), false, true, Set.of()) == 2,
            "online translation reserves every target row");
        check(CandidateTranslationPolicy.glossLines(List.of("en", "ja"), true, false, Set.of("ja")) == 2,
            "an installed offline dictionary reserves its target row");
        check(CandidateTranslationPolicy.glossLines(List.of("en", "ja"), false, false, Set.of("ja")) == 0,
            "the offline switch also gates the offline dictionaries");
        Path root = Files.createTempDirectory("msime-offline-glosses");
        Path outside = null;
        try {
            Path resources = Files.createDirectories(root.resolve("resources"));
            Path glosses = Files.createDirectories(root.resolve("offline-glosses"));
            Files.write(glosses.resolve("zh-ja.db"), new byte[] {0});
            Files.write(glosses.resolve("zh-en.db"), new byte[] {0});
            check(CandidateTranslationPolicy.offlineTargets(List.of("en", "ja"), resources.toString())
                    .equals(List.of("ja")), "only installed non-English dictionaries are offline targets");
            check(CandidateTranslationPolicy.offlineTargets(List.of("fr"), resources.toString()).isEmpty(),
                "a missing dictionary is not an offline target");
            outside = Files.createTempFile("msime-offline-gloss-outside", ".db");
            Path linked = glosses.resolve("zh-fr.db");
            Files.createSymbolicLink(linked, outside);
            check(CandidateTranslationPolicy.offlineTargets(List.of("fr"), resources.toString()).isEmpty(),
                "a linked dictionary is not an offline target");
            check(CandidateTranslationPolicy.offlineTargets(List.of("ja"), "").isEmpty(),
                "no resources means no offline targets");
        } finally {
            try (var paths = Files.walk(root)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> path.toFile().delete());
            }
            if (outside != null) Files.deleteIfExists(outside);
        }
        check(CandidateTranslationPolicy.mergeGlosses(List.of("en", "ja"),
                Map.of("en", "hello", "ja", "こんにちは"), Map.of("en", "hi", "ja", "やあ"))
                .equals("hello\nこんにちは"), "offline glosses outrank the account");
        check(CandidateTranslationPolicy.mergeGlosses(List.of("en", "ja"),
                Map.of("ja", "こんにちは"), Map.of("en", "hi"))
                .equals("hi\nこんにちは"), "the account fills targets the dictionaries miss");
        check(CandidateTranslationPolicy.mergeGlosses(List.of("en", "ja"), Map.of(), Map.of("en", "hi", "ja", "やあ"))
                .equals("hi\nやあ"), "without offline glosses the account rows are unchanged");
        check(CandidateTranslationPolicy.mergeGlosses(List.of("ja"), Map.of("en", "hello"), null).isEmpty(),
            "a gloss for a target the user did not choose is not shown");
        check(CandidateTranslationPolicy.renderedGlossLines("hello") == 1,
            "single rendered gloss stays one row");
        check(CandidateTranslationPolicy.renderedGlossLines("hello\nこんにちは") == 2,
            "actual two-row gloss gets two rows");
        check(CandidateTranslationPolicy.renderedGlossLines(null) == 1,
            "missing rendered gloss stays one row");
        check(CandidateTranslationPolicy.renderedOwnRowLines("") == 1
                && CandidateTranslationPolicy.renderedOwnRowLines(null) == 1,
            "a Hanja with no secondary text stays one row");
        check(CandidateTranslationPolicy.renderedOwnRowLines("나라 이름 한") == 2,
            "the 훈음 takes its own row under the Hanja");
        check(CandidateTranslationPolicy.renderedOwnRowLines("나라 이름 한\nKorea\n韓国") == 4,
            "each gloss row follows the 훈음 row");
        check(CandidateTranslationPolicy.reservedGlossRows(0, false) == 0
                && CandidateTranslationPolicy.reservedGlossRows(1, false) == 1
                && CandidateTranslationPolicy.reservedGlossRows(2, false) == 2,
            "every gloss row under the candidate reserves a row of strip height");
        check(CandidateTranslationPolicy.reservedGlossRows(0, true) == 1,
            "Korean reserves the 훈음 row with glosses off");
        check(CandidateTranslationPolicy.reservedGlossRows(1, true) == 1
                && CandidateTranslationPolicy.reservedGlossRows(2, true) == 2,
            "the 훈음 shares the first gloss row, so Korean reserves no more rows than any other scheme");
        // The account endpoint receives candidate words, so only an explicit choice may reach it.
        check(!CandidateTranslationPolicy.accountSelected(true, false, false, false),
            "candidate translations alone never select the account");
        check(!CandidateTranslationPolicy.accountSelected(false, false, false, false),
            "nothing chosen sends nothing");
        check(CandidateTranslationPolicy.accountSelected(true, true, false, false),
            "an explicit account choice selects the account");
        check(!CandidateTranslationPolicy.accountSelected(false, true, false, false),
            "turning candidate translations off overrides the account choice");
        check(!CandidateTranslationPolicy.accountSelected(true, true, true, false),
            "the user's own NiuTrans service wins over the account");
        check(!CandidateTranslationPolicy.accountSelected(true, true, false, true),
            "the user's own custom service wins over the account");
        System.out.println("Android candidate translation language policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
