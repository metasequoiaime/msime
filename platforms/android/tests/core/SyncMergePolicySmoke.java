package app.msime.android;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class SyncMergePolicySmoke {
    public static void main(String[] arguments) {
        // 首次开启：云端为空直接上传；有数据时没有选择就要问，选了才执行。
        check(SyncMergePolicy.firstRun(false, null) == SyncMergePolicy.Mode.UPLOAD, "empty cloud uploads");
        check(SyncMergePolicy.firstRun(true, null) == null, "cloud data needs a choice");
        check(SyncMergePolicy.firstRun(true, SyncMergePolicy.Choice.MERGE) == SyncMergePolicy.Mode.MERGE, "merge");
        check(SyncMergePolicy.firstRun(true, SyncMergePolicy.Choice.USE_CLOUD) == SyncMergePolicy.Mode.DOWNLOAD,
            "use cloud downloads");
        check(SyncMergePolicy.dictionaryFirstRun(true, false, SyncMergePolicy.Choice.MERGE)
            == SyncMergePolicy.Mode.DOWNLOAD, "merging into an empty local dictionary is a download");
        check(SyncMergePolicy.dictionaryFirstRun(true, true, SyncMergePolicy.Choice.MERGE)
            == SyncMergePolicy.Mode.MERGE, "both sides have words: merge");
        check(SyncMergePolicy.dictionaryFirstRun(false, true, null) == SyncMergePolicy.Mode.UPLOAD,
            "empty cloud dictionary uploads");

        // 之后每一轮。
        check(SyncMergePolicy.incremental(false, false) == SyncMergePolicy.Mode.NONE, "nothing changed");
        check(SyncMergePolicy.incremental(false, true) == SyncMergePolicy.Mode.UPLOAD, "local only");
        check(SyncMergePolicy.incremental(true, false) == SyncMergePolicy.Mode.DOWNLOAD, "cloud only");
        check(SyncMergePolicy.incremental(true, true) == SyncMergePolicy.Mode.MERGE, "both");
        check(SyncMergePolicy.cloudChanged("", 3), "no cursor counts as changed");
        check(!SyncMergePolicy.cloudChanged("3", 3), "same revision is unchanged");
        check(SyncMergePolicy.cloudChanged("2", 3), "older cursor is changed");

        // 常用语：按 id 合并，preferred 胜出；按正文去重；重排 position。
        List<SyncMergePolicy.Phrase> local = List.of(
            new SyncMergePolicy.Phrase("a", "在路上", "", 0),
            new SyncMergePolicy.Phrase("b", "好的", "", 1));
        List<SyncMergePolicy.Phrase> cloud = List.of(
            new SyncMergePolicy.Phrase("a", "旧的正文", "工作", 0),
            new SyncMergePolicy.Phrase("c", "好的", "", 1),
            new SyncMergePolicy.Phrase("d", "马上到", "出行", 2),
            new SyncMergePolicy.Phrase("e", "  ", "", 3));
        List<SyncMergePolicy.Phrase> merged = SyncMergePolicy.mergePhrases(local, cloud);
        check(merged.size() == 3, "merge drops duplicate text and blank: " + merged);
        check("在路上".equals(merged.get(0).text()) && "好的".equals(merged.get(1).text())
            && "马上到".equals(merged.get(2).text()), "local first, then cloud-only");
        check("b".equals(merged.get(1).id()), "first occurrence of a text is kept");
        for (int index = 0; index < merged.size(); index++) check(merged.get(index).position() == index, "positions");
        List<SyncMergePolicy.Phrase> many = new ArrayList<>();
        for (int index = 0; index < 600; index++) many.add(new SyncMergePolicy.Phrase("p" + index, "t" + index, "", index));
        check(SyncMergePolicy.normalized(many).size() == SyncMergePolicy.MAX_PHRASES, "truncated to 500");
        check(SyncMergePolicy.normalized(List.of(new SyncMergePolicy.Phrase("x", "a".repeat(2001), "", 0))).isEmpty(),
            "overlong text dropped");

        Map<String, String> present = new LinkedHashMap<>();
        present.put("l1", "在路上");
        present.put("l2", "删掉我");
        SyncMergePolicy.LocalPlan plan = SyncMergePolicy.localPlan(present, merged);
        check(plan.add().equals(List.of("好的", "马上到")), "plan adds missing texts: " + plan.add());
        check(plan.remove().equals(List.of("l2")), "plan removes texts the target lacks");

        // 只有本机改动时的上传：本机收不下的云端常用语原样带上，本机删掉的照常从云端消失。
        List<SyncMergePolicy.Phrase> ownList = new ArrayList<>();
        for (int index = 0; index < 150; index++) ownList.add(new SyncMergePolicy.Phrase("l" + index, "本机" + index, "", index));
        List<SyncMergePolicy.Phrase> cloudList = new ArrayList<>();
        Set<String> unheld = new HashSet<>();
        for (int index = 0; index < 150; index++) {
            cloudList.add(new SyncMergePolicy.Phrase("c" + index, "云端" + index, "组", index));
            if (index < 100) unheld.add("云端" + index);
        }
        List<SyncMergePolicy.Phrase> upload = SyncMergePolicy.uploadPhrases(ownList, cloudList, unheld);
        check(upload.size() == 250, "local plus unheld cloud phrases: " + upload.size());
        check("本机0".equals(upload.get(0).text()) && "云端0".equals(upload.get(150).text()), "local first, then unheld");
        boolean droppedDeleted = true;
        for (SyncMergePolicy.Phrase phrase : upload) {
            if ("云端120".equals(phrase.text())) droppedDeleted = false;
        }
        check(droppedDeleted, "a cloud phrase this device once held and deleted is not re-uploaded");
        for (int index = 0; index < upload.size(); index++) check(upload.get(index).position() == index, "upload positions");
        check(SyncMergePolicy.uploadPhrases(ownList, cloudList, Set.of()).size() == 150,
            "without unheld phrases the upload is the local list");

        // 设置：本机覆盖云端，本地专属键永远不上传。
        Map<String, Object> base = new LinkedHashMap<>();
        base.put("input.learning", true);
        base.put("general.app_theme", "chunya");
        Map<String, Object> later = new LinkedHashMap<>();
        later.put("general.app_theme", "dongxue");
        later.put("platform.android.touch_incognito", true);
        later.put("developer_options.enabled", true);
        later.put("diagnostic_log.mobile", true);
        later.put("voice_input.contribute_audio", true);
        Map<String, Object> overlay = SyncMergePolicy.overlaySettings(base, later);
        check("dongxue".equals(overlay.get("general.app_theme")), "later write wins");
        check(Boolean.TRUE.equals(overlay.get("input.learning")), "cloud-only key kept");
        check(overlay.size() == 2, "local-only keys stripped: " + overlay.keySet());
        check(SyncMergePolicy.localOnly(null), "null key is never uploaded");

        // 皮肤库的字节预算。
        check(SyncMergePolicy.skinBudget(0) == SyncMergePolicy.SKIN_FIELD_LIMIT, "field limit caps the budget");
        long room = SyncMergePolicy.DOCUMENT_LIMIT_BYTES - 800_000L - SyncMergePolicy.SKINS_KEY.length() - 8L;
        check(SyncMergePolicy.skinBudget(800_000) == room, "document remainder is the budget");
        check(SyncMergePolicy.skinBudget(2_000_000) == 0, "no room left");
        List<long[]> designs = List.of(new long[] {1, 10}, new long[] {2, 30}, new long[] {3, 20});
        List<long[]> sorted = SyncMergePolicy.newestFirst(designs, design -> design[1]);
        check(sorted.get(0)[0] == 2 && sorted.get(1)[0] == 3 && sorted.get(2)[0] == 1, "newest first");
        check(SyncMergePolicy.skinsTrimmed(3, 2) && !SyncMergePolicy.skinsTrimmed(3, 3), "trim notice");

        // 词库合并的分批与种类换算。
        check("quickPhrase".equals(SyncMergePolicy.personalKind("quick_phrase")), "quick phrase kind");
        check("quickPhrase".equals(SyncMergePolicy.personalKind("quick")), "snapshot quick phrase kind");
        check("wubi98".equals(SyncMergePolicy.personalKind("wubi98")), "wubi98 kind");
        check(SyncMergePolicy.personalKind("cangjie") == null, "unknown kind");
        List<SyncMergePolicy.Word> words = new ArrayList<>();
        for (int index = 0; index < 300; index++) words.add(new SyncMergePolicy.Word("pinyin", "k" + index, "词" + index, 1));
        words.add(new SyncMergePolicy.Word("pinyin", "k0", "词0", 9));
        words.add(new SyncMergePolicy.Word("cangjie", "x", "y", 1));
        words.add(new SyncMergePolicy.Word("quick_phrase", "zjd", "在家等", 1));
        List<List<SyncMergePolicy.Word>> batches = SyncMergePolicy.batches(words, SyncMergePolicy.PERSONAL_IMPORT_BATCH);
        int total = 0;
        for (List<SyncMergePolicy.Word> batch : batches) {
            check(batch.size() <= 128, "batch size");
            total += batch.size();
        }
        check(total == 301 && batches.size() == 3, "deduped and batched: " + total);
        check("quickPhrase".equals(batches.get(2).get(batches.get(2).size() - 1).kind()), "kind converted");
        check(SyncMergePolicy.uploadBlocked(1) && !SyncMergePolicy.uploadBlocked(0), "pending queue blocks upload");

        // 节流：满 5 分钟，或有改动时满 30 秒；时钟回拨当作到期。
        long now = 10_000_000L;
        check(SyncMergePolicy.due(now, 0, false), "never attempted");
        check(!SyncMergePolicy.due(now, now - 60_000L, false), "one minute is too soon");
        check(SyncMergePolicy.due(now, now - 60_000L, true), "dirty after debounce");
        check(!SyncMergePolicy.due(now, now - 10_000L, true), "dirty within debounce");
        check(SyncMergePolicy.due(now, now - SyncMergePolicy.THROTTLE_MILLIS, false), "throttle elapsed");
        check(SyncMergePolicy.due(now, now + 1, false), "clock moved back");
        System.out.println("Android sync merge policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
