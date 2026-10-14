import app.msime.android.CandidateGlossPolicy;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.CandidateTranslationRoute;
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
        // 一行里用分号连起来的几个义项要能分别上屏（#6997）：长按「我的」只给一个「my; mine」，点了连分号一起上屏。
        check(CandidateTranslationPolicy.insertionGlosses("my; mine").equals(List.of("my", "mine")),
            "ASCII semicolon separates insertable senses");
        check(CandidateTranslationPolicy.insertionGlosses("苹果；家伙").equals(List.of("苹果", "家伙")),
            "full-width semicolon separates insertable senses");
        check(CandidateTranslationPolicy.insertionGlosses(" ; ；").isEmpty(),
            "whitespace-only senses are not insertable");
        check(CandidateTranslationPolicy.insertionGlosses("a;;b").equals(List.of("a", "b"))
                && CandidateTranslationPolicy.insertionGlosses(";; a ;").equals(List.of("a")),
            "empty senses are dropped");
        check(CandidateTranslationPolicy.insertionGlosses("my; my").equals(List.of("my")),
            "a repeated sense is offered once");
        check(CandidateTranslationPolicy.insertionGlosses("my; mine\nmon; ma")
                .equals(List.of("my", "mine", "mon", "ma")),
            "target rows keep their order and each row splits into its senses");
        check(CandidateTranslationPolicy.insertionGlosses("my; mine\nmon")
                .equals(List.of("my", "mine", "mon")),
            "a line break stays the target separator, never part of a sense");
        check(CandidateTranslationPolicy.insertionGlosses("hello; hi\nhello")
                .equals(List.of("hello", "hi")),
            "a sense shared by two targets is offered once");
        check(CandidateTranslationPolicy.insertionGlosses("1;2;3;4;5;6;7;8;9;10")
                .equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9")),
            "the long-press menu offers at most nine senses");
        check(CandidateTranslationPolicy.insertionGlosses("1;2;3;4;5;6;7;8;9;10\nb")
                .equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "b")),
            "a long first target still leaves the second target a menu item");
        check(CandidateTranslationPolicy.insertionGlosses("1;2;3;4;5;6;7;8;9;10\n1")
                .equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9")),
            "a duplicate secondary sense does not waste a menu slot");
        check(CandidateTranslationPolicy.insertionGlosses("1;2;3;4;5;6;7;8;9;10\n")
                .equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9")),
            "an empty secondary row does not waste a menu slot");
        check(CandidateTranslationPolicy.sourceGetsNoGloss(6)
                && CandidateTranslationPolicy.sourceGetsNoGloss(7)
                && CandidateTranslationPolicy.sourceGetsNoGloss(13)
                && CandidateTranslationPolicy.sourceGetsNoGloss(14),
            "all gloss-blocked sources are recognized by the display gate");
        check(!CandidateTranslationPolicy.userTranslationSourceAllowed(6)
                && !CandidateTranslationPolicy.userTranslationSourceAllowed(7)
                && !CandidateTranslationPolicy.userTranslationSourceAllowed(13)
                && !CandidateTranslationPolicy.userTranslationSourceAllowed(14),
            "emoji, kaomoji, mixed Japanese, and plugin candidates are not sent to user services");
        check(CandidateTranslationPolicy.userTranslationSourceAllowed(0)
                && CandidateTranslationPolicy.userTranslationSourceAllowed(12),
            "ordinary candidate sources remain eligible for user services");
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
            // 随包没有的语言从 state_root 下已下载的 offline-glosses 资源包里找；随包的那份仍然算数。
            Path state = root.resolve("state");
            Path pack = Files.createDirectories(state.resolve("resource-packs/offline-glosses"));
            Files.write(pack.resolve("zh-de.db"), new byte[] {0});
            check(CandidateTranslationPolicy.offlineTargets(List.of("ja", "de", "es"), resources.toString(),
                    state.toString()).equals(List.of("ja", "de")),
                "a downloaded pack adds its languages after the packaged ones");
            check(CandidateTranslationPolicy.offlineTargets(List.of("de"), resources.toString()).isEmpty(),
                "without a state root the downloaded pack is not consulted");
            check(CandidateTranslationPolicy.offlineTargets(List.of("de"), resources.toString(), "state").isEmpty(),
                "a relative state root is ignored");
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
        // #6997：辅助码和释义共用一行，候选条为释义预留的行数不用知道辅助码在不在，切方案、开关辅助码时键盘不变高。
        check(CandidateTranslationPolicy.renderedOwnRowLines(CandidateGlossPolicy.annotation("(ab)", "my; mine", true))
                == CandidateTranslationPolicy.renderedOwnRowLines(CandidateGlossPolicy.annotation("", "my; mine", true)),
            "a helpcode before the gloss adds no row");
        check(CandidateTranslationPolicy.renderedOwnRowLines(CandidateGlossPolicy.annotation("(ab)", "my\nmon", true))
                == CandidateTranslationPolicy.renderedOwnRowLines(CandidateGlossPolicy.annotation("", "my\nmon", true)),
            "a helpcode before two target glosses adds no row");
        check(CandidateTranslationPolicy.reservedGlossRows(0, false) == 0
                && CandidateTranslationPolicy.reservedGlossRows(1, false) == 1
                && CandidateTranslationPolicy.reservedGlossRows(2, false) == 2,
            "every gloss row under the candidate reserves a row of strip height");
        check(CandidateTranslationPolicy.reservedGlossRows(0, true) == 1,
            "Korean reserves the 훈음 row with glosses off");
        check(CandidateTranslationPolicy.reservedGlossRows(1, true) == 1
                && CandidateTranslationPolicy.reservedGlossRows(2, true) == 2,
            "the 훈음 shares the first gloss row, so Korean reserves no more rows than any other scheme");
        // 水杉账号会收到候选词，只有明确选了它才走；用户自己的服务（含凭据可用的腾讯）总是优先。
        check(!account(true, false, false, false, false),
            "candidate translations alone never select the account");
        check(!account(false, false, false, false, false),
            "nothing chosen sends nothing");
        check(account(true, true, false, false, false),
            "an explicit account choice selects the account");
        check(!account(false, true, false, false, false),
            "turning candidate translations off overrides the account choice");
        check(!account(true, true, true, false, false),
            "the user's own NiuTrans service wins over the account");
        check(!account(true, true, false, true, false),
            "the user's own custom service wins over the account");
        check(!account(true, true, false, false, true),
            "usable Tencent credentials win over the account");
        CandidateTranslationRoute none = CandidateTranslationRoute.NONE;
        check(CandidateTranslationPolicy.displayInvalidated(true, false, true, true,
                true, true, List.of("en"), List.of("en"), none, none),
            "turning off offline glosses clears the displayed rows");
        check(CandidateTranslationPolicy.displayInvalidated(true, true, true, true,
                true, true, List.of("en"), List.of("ja"), none, none),
            "changing translation targets clears the displayed rows");
        check(!CandidateTranslationPolicy.displayInvalidated(true, true, true, true,
                true, true, List.of("en"), List.of("en"), none, none),
            "unrelated preference refresh keeps the displayed rows");
        check(CandidateTranslationPolicy.displayInvalidated(true, true, true, true,
                false, false, List.of("en"), List.of("en"), none, customRoute("key")),
            "choosing a translation service clears the displayed rows");
        check(CandidateTranslationPolicy.displayInvalidated(true, true, true, true,
                false, false, List.of("en"), List.of("en"), customRoute("old"), customRoute("new")),
            "changing a service credential clears the displayed rows");
        check(!CandidateTranslationPolicy.displayInvalidated(true, true, true, true,
                false, false, List.of("en"), List.of("en"), customRoute("key"), customRoute("key")),
            "rereading the same service keeps the displayed rows");
        check(CandidateTranslationPolicy.acceptsBinding("7:account-a", "7:account-a"),
            "a translation response remains valid for the same account binding");
        check(CandidateTranslationPolicy.acceptsBinding("7:", "7:"),
            "the anonymous translation identity still has a stable binding snapshot");
        check(!CandidateTranslationPolicy.acceptsBinding("7:account-a", "8:account-a")
                && !CandidateTranslationPolicy.acceptsBinding("7:account-a", "7:account-b")
                && !CandidateTranslationPolicy.acceptsBinding("7:account-a", null),
            "a binding change or unavailable account rejects a stale translation response");
        System.out.println("Android candidate translation language policy passed");
    }

    private static boolean account(boolean translations, boolean account, boolean niutrans,
            boolean custom, boolean tencent) {
        return CandidateTranslationPolicy.route(translations, account, niutrans, "app", "key",
            custom, "https://translate.example/api", "", tencent, "AKIDsynthetic", "synthetic", "")
            .provider() == CandidateTranslationRoute.Provider.ACCOUNT;
    }

    private static CandidateTranslationRoute customRoute(String key) {
        return CandidateTranslationPolicy.route(true, false, false, "", "", true,
            "https://translate.example/api", key, false, "", "", "");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
