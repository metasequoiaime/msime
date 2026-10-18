package app.msime.android;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;
import java.util.ArrayList;
import java.util.List;

/** 本地备份恢复读词库快照的那部分：种类换算、去重分批、权重、单行与总记录数的上限。 */
public final class DictionarySnapshotWordsSmoke {
    public static void main(String[] arguments) throws Exception {
        check("quickPhrase".equals(DictionarySnapshotWords.personalKind("quick_phrase")), "quick phrase kind");
        check("quickPhrase".equals(DictionarySnapshotWords.personalKind("quick")), "snapshot quick phrase kind");
        check("wubi98".equals(DictionarySnapshotWords.personalKind("wubi98")), "wubi98 kind");
        check(DictionarySnapshotWords.personalKind("cangjie") == null, "unknown kind");

        List<DictionarySnapshotWords.Word> words = new ArrayList<>();
        for (int index = 0; index < 300; index++)
            words.add(new DictionarySnapshotWords.Word("pinyin", "k" + index, "词" + index, 1));
        words.add(new DictionarySnapshotWords.Word("pinyin", "k0", "词0", 9));
        words.add(new DictionarySnapshotWords.Word("cangjie", "x", "y", 1));
        words.add(new DictionarySnapshotWords.Word("quick_phrase", "zjd", "在家等", 1));
        List<List<DictionarySnapshotWords.Word>> batches = DictionarySnapshotWords.batches(words, 128);
        int total = 0;
        for (List<DictionarySnapshotWords.Word> batch : batches) {
            check(batch.size() <= 128, "batch size");
            total += batch.size();
        }
        check(total == 301 && batches.size() == 3, "deduped and batched: " + total);
        check("quickPhrase".equals(batches.get(2).get(batches.get(2).size() - 1).kind()), "kind converted");

        check(DictionarySnapshotWords.strictWeight(100L) == 100L, "integer snapshot weight");
        check(DictionarySnapshotWords.strictWeight(1.5d) == null, "fractional snapshot weight is rejected");
        check(DictionarySnapshotWords.strictWeight(0L) == null, "zero snapshot weight is rejected");

        // 也必须拒绝超长 NDJSON 行，不能先用 readLine 把整行分配进内存。
        try {
            DictionarySnapshotWords.readLine(new BufferedReader(new StringReader("x".repeat(70_000) + "\n")));
            throw new AssertionError("oversized snapshot line must be refused");
        } catch (IOException expected) {
            check("snapshot line too large".equals(expected.getMessage()), "oversized snapshot line is bounded");
        }

        // 原生快照最多 500,000 条记录，这里也拒绝超量输入。
        DictionarySnapshotWords.RecordReader records = new DictionarySnapshotWords.RecordReader(
            new BufferedReader(new StringReader("{}\n".repeat(500_002))));
        for (int index = 0; index < 500_001; index++) {
            check("{}".equals(records.next()), "snapshot record within limit");
        }
        try {
            records.next();
            throw new AssertionError("excess snapshot records must be refused");
        } catch (IOException expected) {
            check("snapshot has too many records".equals(expected.getMessage()), "excess snapshot records are bounded");
        }
        System.out.println("Android dictionary snapshot words passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
