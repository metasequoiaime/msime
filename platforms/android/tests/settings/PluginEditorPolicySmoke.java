import app.msime.android.policy.PluginEditorPolicy;
import app.msime.android.policy.PluginEditorPolicy.AuthoredGroup;
import app.msime.android.policy.PluginEditorPolicy.AuthoredPack;
import app.msime.android.policy.PluginEditorPolicy.BulkRows;
import app.msime.android.policy.PluginEditorPolicy.Command;
import app.msime.android.policy.PluginEditorPolicy.Draft;
import app.msime.android.policy.PluginEditorPolicy.DraftGroup;
import app.msime.android.policy.PluginEditorPolicy.DraftWord;
import app.msime.android.policy.PluginEditorPolicy.Phrase;
import java.util.List;

/**
 * 插件编辑器的规则：与共享设置页 `plugin-editor-model.ts` 逐条对应的校验句子、草稿整理、批量粘贴和保存后的提醒。用例取自 Windows 那份移植的 `platforms/windows/tests/ui/plugin_page_model.cpp`，两边同一组输入得到同一句话。
 */
public final class PluginEditorPolicySmoke {
    public static void main(String[] arguments) {
        Draft fresh = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.PHRASE_TABLE);
        check(fresh.license.equals("CC0-1.0") && fresh.version.equals("1.0.0") && fresh.id == null
            && fresh.phrases.size() == 1 && fresh.commands.isEmpty(), "a new draft has the defaults and one blank row");
        issue(fresh, "请给插件起个名字。", "a name comes first");
        check(PluginEditorPolicy.isAuthoredKind("wordbook") && !PluginEditorPolicy.isAuthoredKind("sound")
            && PluginEditorPolicy.AUTHORED_KINDS.size() == 4, "the four text kinds");

        // 单词本的名字和单词按词书的上限，空行不保存。
        Draft words = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.WORDBOOK);
        words.name = "x".repeat(65);
        replace(words.words, List.of(new DraftWord("apple", " ", "苹果"), new DraftWord("", "", "")));
        issue(words, "单词本的名字不能超过 64 个字。", "wordbook name limit");
        words.name = "合成";
        issue(words, null, "a valid wordbook");
        AuthoredPack savedWords = PluginEditorPolicy.packFromDraft(words);
        check(savedWords.words().size() == 1 && savedWords.words().get(0).word().equals("apple")
            && savedWords.words().get(0).phonetic() == null && savedWords.words().get(0).gloss().equals("苹果")
            && savedWords.phrases() == null, "blank rows and blank phonetics are dropped");
        Draft tagged = words.copy();
        replace(tagged.words, List.of(new DraftWord("#tag", "", "x")));
        issue(tagged, "第 1 个单词不能以 # 开头。", "a word cannot start with #");
        replace(tagged.words, List.of(new DraftWord("a", "", "x"), new DraftWord("a", "", "y")));
        issue(tagged, "「a」出现了不止一次。", "duplicate words");

        Draft phrases = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.PHRASE_TABLE);
        phrases.name = "合成";
        issue(phrases, "至少要有一条短语。", "a phrase table needs a phrase");
        replace(phrases.phrases, List.of(new Phrase("DH", "电话")));
        issue(phrases, "第 1 条短语的编码只能是 1 到 32 个小写字母。", "phrase keys are lowercase letters");
        replace(phrases.phrases, List.of(new Phrase("dh", "电话"), new Phrase("", ""), new Phrase("dh", " 电话 ")));
        issue(phrases, "第 3 条短语和前面的一条重复了。", "duplicates compare trimmed, row numbers count blank rows");
        replace(phrases.phrases, List.of(new Phrase("dh", "电话")));
        phrases.license = "随便用";
        issue(phrases, "许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。", "licenses are SPDX codes");
        phrases.license = "CC-BY-4.0";
        phrases.author = "甲\n";
        issue(phrases, "作者太长了。", "control characters in the author");
        phrases.author = "";
        replace(phrases.phrases, List.of(new Phrase("dh", "x".repeat(200))));
        issue(phrases, "第 1 条短语的文字太长了。", "phrase text limit");
        replace(phrases.phrases, List.of(new Phrase("dh", "a\tb")));
        issue(phrases, "第 1 条短语的文字里不能有换行或制表符。", "phrase text cannot hold a tab");
        phrases.name = "中".repeat(27);
        issue(phrases, "名字太长了。", "names are limited in UTF-8 bytes");
        phrases.name = "　合成﻿";
        replace(phrases.phrases, List.of(new Phrase("dh", "电话")));
        issue(phrases, null, "ideographic spaces and BOMs trim as in JavaScript");
        check(PluginEditorPolicy.packFromDraft(phrases).name().equals("合成"), "and are trimmed when saving");
        phrases.version = " ";
        issue(phrases, "请填写版本，例如 1.0.0。", "a version is needed");

        Draft commands = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.COMMAND_TABLE);
        commands.name = "合成";
        replace(commands.commands, List.of(new Command("x", "x", "{clipboard}")));
        issue(commands, "第 1 条指令的花括号里只能写 {date}、{time} 或 {weekday}。", "unknown placeholders");
        replace(commands.commands, List.of(new Command("sig", "签名", "{date:%m月%d日} {weekday} {time}"),
            new Command("sig", "x", "y")));
        issue(commands, "指令 /sig 重复了。", "duplicate triggers");
        commands.commands.remove(1);
        issue(commands, null, "known placeholders, with formats");
        replace(commands.commands, List.of(new Command("sig", "", "y")));
        issue(commands, "第 1 条指令还没有填写标题。", "a command needs a title");

        Draft symbols = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.SYMBOL_SET);
        symbols.name = "合成";
        replace(symbols.groups, List.of(new DraftGroup("symbols", "同名", "", "→"), new DraftGroup("symbols", "同名", "", "←")));
        issue(symbols, "第 2 组的标题和同一页的另一组重复了。", "group titles are unique per tab");
        symbols.groups.set(1, new DraftGroup("kaomoji", "同名", "", "←"));
        issue(symbols, null, "the same title on another tab");
        replace(symbols.groups, List.of(new DraftGroup("kaomoji", "开心", "", "(^_^)\n\n  ( ﾟ∀ﾟ)  \n"),
            new DraftGroup("symbols", "箭头", "jiantou", "→\n→")));
        issue(symbols, "第 2 组里「→」重复了。", "duplicate symbols");
        symbols.groups.set(1, new DraftGroup("symbols", "箭头", "jiantou", "→\r\n←"));
        AuthoredPack savedSymbols = PluginEditorPolicy.packFromDraft(symbols);
        check(savedSymbols.groups().size() == 2 && savedSymbols.groups().get(0).tab().equals("kaomoji")
            && savedSymbols.groups().get(0).items().equals(List.of("(^_^)", "( ﾟ∀ﾟ)"))
            && savedSymbols.groups().get(0).keywords() == null
            && "jiantou".equals(savedSymbols.groups().get(1).keywords())
            && savedSymbols.groups().get(1).items().equals(List.of("→", "←")),
            "symbols are one per line, trimmed, blank lines skipped");

        // 读出的包变成草稿，再整理回去不变。
        AuthoredPack loaded = new AuthoredPack("symbol_set", "my-1a2b3c4d", "合成", "2.0.0", "CC-BY-4.0", "甲", null,
            null, null, List.of(new AuthoredGroup("symbols", "箭头", null, List.of("→", "←"))), null);
        Draft draft = PluginEditorPolicy.draftFromPack(loaded);
        check("my-1a2b3c4d".equals(draft.id) && draft.author.equals("甲") && draft.description.isEmpty()
            && draft.groups.size() == 1 && draft.groups.get(0).items().equals("→\n←")
            && draft.groups.get(0).keywords().isEmpty(), "a loaded pack becomes a draft");
        AuthoredPack again = PluginEditorPolicy.packFromDraft(draft);
        check("my-1a2b3c4d".equals(again.id()) && "甲".equals(again.author()) && again.description() == null
            && again.groups().get(0).items().equals(loaded.groups().get(0).items()), "and saves back unchanged");
        Draft changed = draft.copy();
        check(changed.equals(draft), "an untouched draft is not dirty");
        changed.groups.set(0, new DraftGroup("symbols", "箭头", "", "→\n←\n↑"));
        check(!changed.equals(draft) && draft.groups.get(0).items().equals("→\n←"), "an edited copy is dirty and leaves the original");

        // 批量粘贴。
        BulkRows<Phrase> pasted = PluginEditorPolicy.parsePhraseLines("dz  合成 地址 \n\n dh 电话");
        check(pasted.issue() == null && pasted.rows().size() == 2 && pasted.rows().get(0).equals(new Phrase("dz", "合成 地址"))
            && pasted.rows().get(1).equals(new Phrase("dh", "电话")), "phrase lines: key, then the rest of the line");
        check("第 2 行要写成「编码 文字」，中间用空格隔开。".equals(
            PluginEditorPolicy.parsePhraseLines("dh 电话\r\nnospace").issue()), "a phrase line without a space");
        BulkRows<Phrase> blankText = PluginEditorPolicy.parsePhraseLines("dz   ");
        Draft onlyKey = PluginEditorPolicy.emptyDraft(PluginEditorPolicy.PHRASE_TABLE);
        onlyKey.name = "合成";
        replace(onlyKey.phrases, blankText.rows());
        check(blankText.issue() == null && blankText.rows().size() == 1, "only spaces after the key still read as a row");
        issue(onlyKey, "第 1 条短语还没有填写文字。", "as the regular expression does");
        BulkRows<DraftWord> pastedWords = PluginEditorPolicy.parseWordLines("# 合成词表\napple\t/ˈæp.əl/\tn. 苹果\nbanana\tn. 香蕉\n");
        check(pastedWords.issue() == null && pastedWords.rows().size() == 2
            && pastedWords.rows().get(0).phonetic().equals("/ˈæp.əl/") && pastedWords.rows().get(1).phonetic().isEmpty()
            && pastedWords.rows().get(1).gloss().equals("n. 香蕉"), "word lines: two or three tab separated columns");
        String four = PluginEditorPolicy.parseWordLines("a\tb\tc\td").issue();
        check(four != null && four.startsWith("第 1 行"), "four columns are refused");
        List<Phrase> appended = PluginEditorPolicy.appendRows(fresh.phrases, pasted.rows(), PluginEditorPolicy::isBlankPhrase);
        check(appended.size() == 2 && appended.get(0).key().equals("dz"), "pasted rows replace a lone blank row");
        check(PluginEditorPolicy.appendRows(appended, pasted.rows(), PluginEditorPolicy::isBlankPhrase).size() == 4,
            "and are appended after real rows");

        check(!PluginEditorPolicy.templatePlaceholdersKnown("{date")
            && !PluginEditorPolicy.templatePlaceholdersKnown("}")
            && !PluginEditorPolicy.templatePlaceholdersKnown("{{date}}")
            && !PluginEditorPolicy.templatePlaceholdersKnown("{dates}")
            && PluginEditorPolicy.templatePlaceholdersKnown("无占位符"), "braces pair up without nesting");

        // 保存之后的提醒：去处换成手机上的。
        check(PluginEditorPolicy.authoredSaveNote("phrase_table", true, true).equals("已保存，修改已经生效。")
            && PluginEditorPolicy.authoredSaveNote("command_table", true, false).equals("已保存。/ 指令现在关着，在下面打开后才能用。")
            && PluginEditorPolicy.authoredSaveNote("wordbook", false, false).equals("已保存，这本书出现在背单词的词书里。")
            && PluginEditorPolicy.authoredSaveNote("symbol_set", false, false).equals("已保存，符号面板里会显示这些符号。")
            && !PluginEditorPolicy.authoredSaveNote("phrase_table", false, true).contains("Shift"),
            "what is still missing after a save");
        System.out.println("PluginEditorPolicySmoke ok");
    }

    private static <T> void replace(List<T> rows, List<T> values) {
        rows.clear();
        rows.addAll(values);
    }

    private static void issue(Draft draft, String expected, String message) {
        String actual = PluginEditorPolicy.draftIssue(draft);
        if (expected == null ? actual != null : !expected.equals(actual)) {
            throw new AssertionError(message + ": " + actual);
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
