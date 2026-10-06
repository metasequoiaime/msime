import app.msime.android.DictionaryCollectionsStore;
import java.util.List;

public final class DictionaryCollectionsStoreSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(DictionaryCollectionsStore.validName("工作"));
        check(DictionaryCollectionsStore.validName("字".repeat(32)));
        check(!DictionaryCollectionsStore.validName("字".repeat(33)));
        check(!DictionaryCollectionsStore.validName(""));
        check(!DictionaryCollectionsStore.validName(" 工作"));
        check(!DictionaryCollectionsStore.validName("工\n作"));
        check(DictionaryCollectionsStore.validName("😀".repeat(32)));

        check(DictionaryCollectionsStore.validPinyin("shui'shan"));
        check(DictionaryCollectionsStore.validPinyin("pinyin"));
        check(!DictionaryCollectionsStore.validPinyin("'pin"));
        check(!DictionaryCollectionsStore.validPinyin("pin'"));
        check(!DictionaryCollectionsStore.validPinyin("pin''yin"));
        check(!DictionaryCollectionsStore.validPinyin("Pin"));
        check(!DictionaryCollectionsStore.validPinyin(""));
        check(DictionaryCollectionsStore.normalizePinyin(" Shui Shan ").equals("shui'shan"));
        check(DictionaryCollectionsStore.normalizePinyin("hou’xuan").equals("hou'xuan"));
        check(DictionaryCollectionsStore.displayCode("hou'xuan'xiang").equals("hou’xuan’xiang"));

        check(DictionaryCollectionsStore.countLabel(128406).equals("128,406 条"));
        check(DictionaryCollectionsStore.countLabel(-3).equals("0 条"));

        check(DictionaryCollectionsStore.nameFromFile("网络流行语.txt").equals("网络流行语"));
        check(DictionaryCollectionsStore.nameFromFile("luna_pinyin.dict.yaml").equals("luna_pinyin"));
        check(DictionaryCollectionsStore.nameFromFile(".txt").equals("导入的词库"));
        check(DictionaryCollectionsStore.nameFromFile("字".repeat(40) + ".txt").equals("字".repeat(32)));
        check(DictionaryCollectionsStore.formatForFile("a.scel", "txt").equals("scel"));
        check(DictionaryCollectionsStore.formatForFile("a.dict.yaml", "txt").equals("rime"));
        check(DictionaryCollectionsStore.formatForFile("a.txt", "hans").equals("hans"));
        check(DictionaryCollectionsStore.binaryFormat("scel") && !DictionaryCollectionsStore.binaryFormat("txt"));

        List<DictionaryCollectionsStore.ImportSource> sources = DictionaryCollectionsStore.importSources(
            List.of("txt", "standard", "windows", "hans", "rime"));
        check(sources.size() == 3);
        check(sources.get(0).format().equals("txt") && sources.get(1).format().equals("rime")
            && sources.get(2).format().equals("hans"));
        check(DictionaryCollectionsStore.importSources(List.of("txt", "scel")).get(1).label().contains(".scel"));
        check(DictionaryCollectionsStore.importSources(List.of()).isEmpty());

        DictionaryCollectionsStore.View view = new DictionaryCollectionsStore.View(List.of(
            new DictionaryCollectionsStore.Collection("c1", "网络流行语", "pinyin", "community", "r1", true, 10, 0),
            new DictionaryCollectionsStore.Collection("c2", "工作", "pinyin", "user", "", false, 0, 0)),
            List.of("txt"), null);
        check(view.installed("r1") && !view.installed("r2"));
        check(view.find("c2").name().equals("工作") && view.find("c3") == null);

        check(DictionaryCollectionsStore.failureMessage("builtin_locked").contains("内置"));
        check(DictionaryCollectionsStore.failureMessage(null).equals(DictionaryCollectionsStore.failureMessage("x")));
        check(DictionaryCollectionsStore.BUILTIN_PINYIN.equals("builtin:pinyin"));
    }
}
