import app.msime.android.home.PreserveEnglishPunctuation;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * 「保留英文的标点」的选择规则（#11890）：21 个键与标点契约第 7 版 `preserveEnglish` 段逐个对上，切换按契约顺序写，不认识的键丢掉，空集合显示「无」。
 */
public final class PreserveEnglishPunctuationSmoke {
    public static void main(String[] arguments) throws Exception {
        List<String> contract = contractKeys(policy());
        List<String> keys = new ArrayList<>();
        for (PreserveEnglishPunctuation.Key key : PreserveEnglishPunctuation.KEYS) keys.add(key.ascii());
        check(keys.equals(contract), "keys follow the contract's preserveEnglish order");
        check(keys.size() == 21, "21 keys");

        check(PreserveEnglishPunctuation.summary(List.of()).equals("无"), "empty set reads 无");
        List<String> one = PreserveEnglishPunctuation.toggled(List.of(), "\\");
        check(one.equals(List.of("\\")), "a toggle adds the key");
        List<String> two = PreserveEnglishPunctuation.toggled(one, "`");
        check(two.equals(List.of("\\", "`")), "the set keeps contract order");
        check(PreserveEnglishPunctuation.summary(two).equals("\\ `"), "the summary lists the keys");
        check(PreserveEnglishPunctuation.toggled(two, "\\").equals(List.of("`")), "a second toggle removes the key");
        check(PreserveEnglishPunctuation.selected(List.of("/", "`", "a", "`")).equals(List.of("`")),
            "unknown and duplicate keys are dropped");
        System.out.println("Android preserve English punctuation policy passed");
    }

    /** 从 user.dir 往上找仓库里的标点契约。 */
    private static Path policy() {
        for (Path dir = Path.of(System.getProperty("user.dir")).toAbsolutePath(); dir != null; dir = dir.getParent()) {
            Path candidate = dir.resolve("shared/contracts/punctuation/policy.json");
            if (Files.isRegularFile(candidate)) return candidate;
        }
        throw new AssertionError("run this smoke from inside the repository; policy.json not found above user.dir");
    }

    /** policy.json 的 preserveEnglish 段。 */
    private static List<String> contractKeys(Path policy) throws Exception {
        String text = Files.readString(policy);
        int start = text.indexOf("\"preserveEnglish\"");
        int open = text.indexOf('[', start);
        int close = text.indexOf("\n  ]", open);
        List<String> keys = new ArrayList<>();
        String body = text.substring(open + 1, close);
        for (int index = 0; index < body.length(); index++) {
            if (body.charAt(index) != '"') continue;
            char value = body.charAt(index + 1);
            if (value == '\\') {
                keys.add(String.valueOf(body.charAt(index + 2)));
                index += 3;
            } else {
                keys.add(String.valueOf(value));
                index += 2;
            }
        }
        return keys;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
