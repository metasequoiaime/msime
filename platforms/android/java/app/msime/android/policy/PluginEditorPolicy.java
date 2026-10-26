package app.msime.android.policy;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 插件页的「新建插件 / 编辑」：草稿、校验、批量粘贴和保存后的提醒，逐条移植自共享设置页的 `packages/ui/src/settings/plugin-editor-model.ts`，Windows 的 `PluginEditorModel.h` 是同一份规则的另一份移植。
 *
 * <p>这里只是让用户在保存前就知道该改哪一行；宿主（client-services 的 `plugins::authoring`，经 `msime_client_plugins` 的 `authoring_save`）保存时按同一套规则再查一遍，两边不一致时以宿主的拒绝为准，页面把它的 `detail` 就地显示。不依赖 Android，也不用 org.json，JSON 的读写留在页面里，所以 JVM 冒烟（`tests/settings/PluginEditorPolicySmoke.java`）能直接跑。
 *
 * <p>长度按 TypeScript 的写法算：「UTF-16」是 `String.length()`，「字节」是 UTF-8 字节数，「个字」是码位数；首尾空白和 `\s` 按 JavaScript 的空白字符集（含 U+00A0、U+3000 和 U+FEFF），不按 Java 的 `trim()`。
 */
public final class PluginEditorPolicy {
    public static final String PHRASE_TABLE = "phrase_table";
    public static final String COMMAND_TABLE = "command_table";
    public static final String SYMBOL_SET = "symbol_set";
    public static final String WORDBOOK = "wordbook";
    public static final String TAB_SYMBOLS = "symbols";
    public static final String TAB_KAOMOJI = "kaomoji";

    /** 「新建插件」里列出的一种类型，带一句给不读文档的人看的说明。 */
    public record KindEntry(String kind, String title, String description) {}

    /** 编辑器能写的类型，按「新建插件」里列出的顺序。 */
    public static final List<KindEntry> AUTHORED_KINDS = List.of(
        new KindEntry(PHRASE_TABLE, "短语表", "在 K 模式里输入编码，打出一段常用文字，例如输入 dz 打出家庭地址。"),
        new KindEntry(COMMAND_TABLE, "指令表", "按 / 再输入指令字母，插入签名、当天日期这类模板文字。"),
        new KindEntry(SYMBOL_SET, "符号集", "给符号面板加几组自己常用的符号或颜文字。"),
        new KindEntry(WORDBOOK, "单词本", "做一本自己的词书，在背单词里复习。"));

    /** 「新建插件」旁边的一句说明。 */
    public static final String CREATE_DESCRIPTION =
        "不用写代码，填好短语、指令、符号或单词就能做出自己的插件，做好后也可以发布到社区";

    /** 新建插件时许可证的默认值。 */
    public static final String DEFAULT_LICENSE = "CC0-1.0";
    /** 新建插件时版本的默认值。 */
    public static final String DEFAULT_VERSION = "1.0.0";

    // 以下上限与 `crates/client-services/src/plugins/` 的清单校验一致。
    private static final int MAX_NAME_BYTES = 80;
    private static final int MAX_VERSION_BYTES = 32;
    private static final int MAX_LICENSE_BYTES = 64;
    private static final int MAX_AUTHOR_BYTES = 120;
    private static final int MAX_DESCRIPTION_BYTES = 500;
    /** `phrase_table::MAX_PHRASES`。 */
    public static final int MAX_PHRASES = 2000;
    /** `command_table::MAX_COMMANDS`。 */
    public static final int MAX_COMMANDS = 256;
    private static final int MAX_KEY_LETTERS = 32;
    /** 短语文字和指令模板的 UTF-16 单元上限：Windows 候选管道的文本字段。 */
    public static final int MAX_TEXT_UTF16 = 199;
    private static final int MAX_COMMAND_TITLE_BYTES = 48;
    /** `symbol_set::MAX_GROUPS`。 */
    public static final int MAX_GROUPS = 32;
    private static final int MAX_GROUP_ITEMS = 512;
    private static final int MAX_SYMBOL_ITEMS = 2048;
    private static final int MAX_GROUP_TITLE_BYTES = 48;
    private static final int MAX_KEYWORDS_BYTES = 256;
    private static final int MAX_ITEM_UTF16 = 64;
    /** `vocabulary::wordbook::MAX_ENTRIES`。 */
    public static final int MAX_WORDS = 20000;
    private static final int MAX_WORD_CHARS = 64;
    private static final int MAX_PHONETIC_CHARS = 64;
    private static final int MAX_GLOSS_CHARS = 256;
    /** 单词本名字的码位上限。 */
    public static final int MAX_WORDBOOK_NAME_CHARS = 64;

    private static final Pattern KEY = Pattern.compile("[a-z]+");
    private static final Pattern LICENSE = Pattern.compile("[A-Za-z0-9.\\-+() ]+");
    /** JavaScript `\s` 的字符集：WhiteSpace 加 LineTerminator。 */
    private static final String JS_SPACE = "\\t\\n\\u000B\\f\\r \\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000\\uFEFF";
    /** `/^\s*(\S+)\s+(.+?)\s*$/`。 */
    private static final Pattern PHRASE_LINE = Pattern.compile(
        "^[" + JS_SPACE + "]*([^" + JS_SPACE + "]+)[" + JS_SPACE + "]+(.+?)[" + JS_SPACE + "]*$");
    private static final Pattern TIMED_PLACEHOLDER = Pattern.compile("(?:date|time)(?::.*)?", Pattern.DOTALL);
    private static final Pattern LINE_BREAK = Pattern.compile("\\r?\\n");

    private PluginEditorPolicy() {}

    /** 短语表的一行：`client-services::plugins::phrase_table::PhraseRow`。 */
    public record Phrase(String key, String text) {}

    /** 指令表的一条：`command_table` 的一项。 */
    public record Command(String trigger, String title, String template) {}

    /** 编辑中的一组符号：`items` 是文本框里的原文，一行一项。 */
    public record DraftGroup(String tab, String title, String keywords, String items) {}

    /** 编辑中的单词：音标留空时就是空串。 */
    public record DraftWord(String word, String phonetic, String gloss) {}

    /** 编辑器里符号集的一组：`authoring::AuthoredGroup`，`keywords` 为 null 表示没写。 */
    public record AuthoredGroup(String tab, String title, String keywords, List<String> items) {}

    /** 编辑器里单词本的一个单词：`authoring::AuthoredWord`，`phonetic` 为 null 表示没写。 */
    public record AuthoredWord(String word, String phonetic, String gloss) {}

    /**
     * 编辑器与宿主交换的一个包：`authoring::AuthoredPack`。`id` 为 null 表示新建、由宿主生成；`author`、`description` 为 null 表示没写；四个内容字段只有与 `kind` 对应的那一个非 null。
     */
    public record AuthoredPack(String kind, String id, String name, String version, String license, String author,
            String description, List<Phrase> phrases, List<Command> commands, List<AuthoredGroup> groups,
            List<AuthoredWord> words) {}

    /** 编辑器的草稿：全部字段都是表单里的原文，保存前由 {@link #packFromDraft} 整理。四种内容都在，只有与 `kind` 对应的那一种会保存。 */
    public static final class Draft {
        public final String kind;
        /** 编辑已安装的包时是它的 id；新建时为 null，由宿主生成。 */
        public final String id;
        public String name;
        public String version;
        public String license;
        public String author;
        public String description;
        public final List<Phrase> phrases;
        public final List<Command> commands;
        public final List<DraftGroup> groups;
        public final List<DraftWord> words;

        public Draft(String kind, String id, String name, String version, String license, String author,
                String description, List<Phrase> phrases, List<Command> commands, List<DraftGroup> groups,
                List<DraftWord> words) {
            this.kind = kind;
            this.id = id;
            this.name = name;
            this.version = version;
            this.license = license;
            this.author = author;
            this.description = description;
            this.phrases = new ArrayList<>(phrases);
            this.commands = new ArrayList<>(commands);
            this.groups = new ArrayList<>(groups);
            this.words = new ArrayList<>(words);
        }

        /** 一份内容相同、互不影响的副本，用来记下打开时的样子，判断有没有改过。 */
        public Draft copy() {
            return new Draft(kind, id, name, version, license, author, description, phrases, commands, groups, words);
        }

        @Override public boolean equals(Object other) {
            if (!(other instanceof Draft that)) return false;
            return kind.equals(that.kind) && Objects.equals(id, that.id) && name.equals(that.name)
                && version.equals(that.version) && license.equals(that.license) && author.equals(that.author)
                && description.equals(that.description) && phrases.equals(that.phrases)
                && commands.equals(that.commands) && groups.equals(that.groups) && words.equals(that.words);
        }

        @Override public int hashCode() {
            return Objects.hash(kind, id, name, version, license, author, description, phrases, commands, groups,
                words);
        }
    }

    /** 这个类型能不能在插件编辑器里编辑。 */
    public static boolean isAuthoredKind(String kind) {
        for (KindEntry entry : AUTHORED_KINDS) if (entry.kind().equals(kind)) return true;
        return false;
    }

    /** 类型的显示名；不是编辑器的类型时是「插件」。 */
    public static String kindTitle(String kind) {
        for (KindEntry entry : AUTHORED_KINDS) if (entry.kind().equals(kind)) return entry.title();
        return "插件";
    }

    public static Phrase emptyPhrase() { return new Phrase("", ""); }

    public static Command emptyCommand() { return new Command("", "", ""); }

    public static DraftGroup emptyGroup() { return new DraftGroup(TAB_SYMBOLS, "", "", ""); }

    public static DraftWord emptyWord() { return new DraftWord("", "", ""); }

    /** 新建一种插件时的草稿：默认许可证和版本，内容先给一个空行。 */
    public static Draft emptyDraft(String kind) {
        return new Draft(kind, null, "", DEFAULT_VERSION, DEFAULT_LICENSE, "", "",
            PHRASE_TABLE.equals(kind) ? List.of(emptyPhrase()) : List.of(),
            COMMAND_TABLE.equals(kind) ? List.of(emptyCommand()) : List.of(),
            SYMBOL_SET.equals(kind) ? List.of(emptyGroup()) : List.of(),
            WORDBOOK.equals(kind) ? List.of(emptyWord()) : List.of());
    }

    /** 宿主读出的已安装包，变成可以编辑的草稿。 */
    public static Draft draftFromPack(AuthoredPack pack) {
        List<DraftGroup> groups = new ArrayList<>();
        if (pack.groups() != null) {
            for (AuthoredGroup group : pack.groups()) {
                groups.add(new DraftGroup(group.tab(), group.title(), orEmpty(group.keywords()),
                    String.join("\n", group.items())));
            }
        }
        List<DraftWord> words = new ArrayList<>();
        if (pack.words() != null) {
            for (AuthoredWord word : pack.words()) words.add(new DraftWord(word.word(), orEmpty(word.phonetic()), word.gloss()));
        }
        return new Draft(pack.kind(), pack.id(), pack.name(), pack.version(), pack.license(), orEmpty(pack.author()),
            orEmpty(pack.description()), pack.phrases() == null ? List.of() : pack.phrases(),
            pack.commands() == null ? List.of() : pack.commands(), groups, words);
    }

    public static boolean isBlankPhrase(Phrase row) { return blank(row.key()) && blank(row.text()); }

    public static boolean isBlankCommand(Command row) {
        return blank(row.trigger()) && blank(row.title()) && blank(row.template());
    }

    public static boolean isBlankGroup(DraftGroup group) {
        return blank(group.title()) && blank(group.keywords()) && blank(group.items());
    }

    public static boolean isBlankWord(DraftWord row) {
        return blank(row.word()) && blank(row.phonetic()) && blank(row.gloss());
    }

    /** 文本框里的符号：一行一项，去掉首尾空白，跳过空行。 */
    public static List<String> groupItems(String text) {
        List<String> items = new ArrayList<>();
        for (String line : LINE_BREAK.split(text, -1)) {
            String item = trim(line);
            if (!item.isEmpty()) items.add(item);
        }
        return items;
    }

    /** 草稿整理成交给宿主的包：各字段去掉首尾空白，可选字段留空时为 null，整行都空着的行不保存。 */
    public static AuthoredPack packFromDraft(Draft draft) {
        List<Phrase> phrases = null;
        List<Command> commands = null;
        List<AuthoredGroup> groups = null;
        List<AuthoredWord> words = null;
        switch (draft.kind) {
            case PHRASE_TABLE -> {
                phrases = new ArrayList<>();
                for (Phrase row : draft.phrases) {
                    if (!isBlankPhrase(row)) phrases.add(new Phrase(trim(row.key()), trim(row.text())));
                }
            }
            case COMMAND_TABLE -> {
                commands = new ArrayList<>();
                for (Command row : draft.commands) {
                    if (!isBlankCommand(row)) {
                        commands.add(new Command(trim(row.trigger()), trim(row.title()), trim(row.template())));
                    }
                }
            }
            case SYMBOL_SET -> {
                groups = new ArrayList<>();
                for (DraftGroup group : draft.groups) {
                    if (!isBlankGroup(group)) {
                        groups.add(new AuthoredGroup(group.tab(), trim(group.title()), optional(group.keywords()),
                            groupItems(group.items())));
                    }
                }
            }
            case WORDBOOK -> {
                words = new ArrayList<>();
                for (DraftWord row : draft.words) {
                    if (!isBlankWord(row)) {
                        words.add(new AuthoredWord(trim(row.word()), optional(row.phonetic()), trim(row.gloss())));
                    }
                }
            }
            default -> { }
        }
        return new AuthoredPack(draft.kind, draft.id, trim(draft.name), trim(draft.version), trim(draft.license),
            optional(draft.author), optional(draft.description), phrases, commands, groups, words);
    }

    /** 指令模板里的占位符是否都认得：`{date}`、`{time}`（都可以带 `:格式`）和 `{weekday}`，花括号成对且不嵌套。格式本身对不对、展开后会不会太长由宿主检查。 */
    public static boolean templatePlaceholdersKnown(String template) {
        String rest = template;
        while (true) {
            int open = firstBrace(rest);
            if (open < 0) return true;
            if (rest.charAt(open) == '}') return false;
            String after = rest.substring(open + 1);
            int close = firstBrace(after);
            if (close < 0 || after.charAt(close) == '{') return false;
            String placeholder = after.substring(0, close);
            if (!placeholder.equals("weekday") && !TIMED_PLACEHOLDER.matcher(placeholder).matches()) return false;
            rest = after.substring(close + 1);
        }
    }

    private static int firstBrace(String text) {
        for (int index = 0; index < text.length(); index++) {
            char value = text.charAt(index);
            if (value == '{' || value == '}') return index;
        }
        return -1;
    }

    /** 草稿现在不能保存的第一个原因，用页面上的说法；能保存时为 null。行号按表单里的行数（整行都空着的行不算问题）。 */
    public static String draftIssue(Draft draft) {
        String name = trim(draft.name);
        if (name.isEmpty()) return "请给插件起个名字。";
        if (bytes(name) > MAX_NAME_BYTES || control(name)) return "名字太长了。";
        if (WORDBOOK.equals(draft.kind) && characters(name) > MAX_WORDBOOK_NAME_CHARS)
            return "单词本的名字不能超过 " + MAX_WORDBOOK_NAME_CHARS + " 个字。";
        String version = trim(draft.version);
        if (version.isEmpty()) return "请填写版本，例如 1.0.0。";
        if (bytes(version) > MAX_VERSION_BYTES || control(version)) return "版本太长了。";
        String license = trim(draft.license);
        if (license.isEmpty()) return "请填写许可证，不确定时写 " + DEFAULT_LICENSE + "。";
        if (bytes(license) > MAX_LICENSE_BYTES || !LICENSE.matcher(license).matches())
            return "许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。";
        if (bytes(trim(draft.author)) > MAX_AUTHOR_BYTES || control(draft.author)) return "作者太长了。";
        if (bytes(trim(draft.description)) > MAX_DESCRIPTION_BYTES || control(draft.description)) return "介绍太长了。";
        return switch (draft.kind) {
            case PHRASE_TABLE -> phrasesIssue(draft.phrases);
            case COMMAND_TABLE -> commandsIssue(draft.commands);
            case SYMBOL_SET -> groupsIssue(draft.groups);
            case WORDBOOK -> wordsIssue(draft.words);
            default -> null;
        };
    }

    private static String phrasesIssue(List<Phrase> rows) {
        Set<String> seen = new HashSet<>();
        int count = 0;
        for (int index = 0; index < rows.size(); index++) {
            Phrase row = rows.get(index);
            if (isBlankPhrase(row)) continue;
            count++;
            String at = "第 " + (index + 1) + " 条短语";
            String key = trim(row.key());
            String text = trim(row.text());
            if (key.isEmpty()) return at + "还没有填写编码。";
            if (key.length() > MAX_KEY_LETTERS || !KEY.matcher(key).matches())
                return at + "的编码只能是 1 到 " + MAX_KEY_LETTERS + " 个小写字母。";
            if (text.isEmpty()) return at + "还没有填写文字。";
            if (text.length() > MAX_TEXT_UTF16) return at + "的文字太长了。";
            if (control(text)) return at + "的文字里不能有换行或制表符。";
            if (!seen.add(key + "\n" + text)) return at + "和前面的一条重复了。";
        }
        if (count == 0) return "至少要有一条短语。";
        if (count > MAX_PHRASES) return "一个短语表最多 " + MAX_PHRASES + " 条。";
        return null;
    }

    private static String commandsIssue(List<Command> rows) {
        Set<String> triggers = new HashSet<>();
        int count = 0;
        for (int index = 0; index < rows.size(); index++) {
            Command row = rows.get(index);
            if (isBlankCommand(row)) continue;
            count++;
            String at = "第 " + (index + 1) + " 条指令";
            String trigger = trim(row.trigger());
            String title = trim(row.title());
            String template = trim(row.template());
            if (trigger.isEmpty()) return at + "还没有填写指令字母。";
            if (trigger.length() > MAX_KEY_LETTERS || !KEY.matcher(trigger).matches())
                return at + "的指令只能是 1 到 " + MAX_KEY_LETTERS + " 个小写字母。";
            if (!triggers.add(trigger)) return "指令 /" + trigger + " 重复了。";
            if (title.isEmpty()) return at + "还没有填写标题。";
            if (bytes(title) > MAX_COMMAND_TITLE_BYTES || control(title)) return at + "的标题太长了。";
            if (template.isEmpty()) return at + "还没有填写要插入的文字。";
            if (template.length() > MAX_TEXT_UTF16) return at + "要插入的文字太长了。";
            if (control(template)) return at + "要插入的文字里不能有换行或制表符。";
            if (!templatePlaceholdersKnown(template))
                return at + "的花括号里只能写 {date}、{time} 或 {weekday}。";
        }
        if (count == 0) return "至少要有一条指令。";
        if (count > MAX_COMMANDS) return "一个指令表最多 " + MAX_COMMANDS + " 条。";
        return null;
    }

    private static String groupsIssue(List<DraftGroup> groups) {
        Set<String> titles = new HashSet<>();
        int count = 0;
        int total = 0;
        for (int index = 0; index < groups.size(); index++) {
            DraftGroup group = groups.get(index);
            if (isBlankGroup(group)) continue;
            count++;
            String at = "第 " + (index + 1) + " 组";
            String title = trim(group.title());
            if (title.isEmpty()) return at + "还没有填写标题。";
            if (bytes(title) > MAX_GROUP_TITLE_BYTES || control(title)) return at + "的标题太长了。";
            if (!titles.add(group.tab() + "\n" + title)) return at + "的标题和同一页的另一组重复了。";
            if (bytes(trim(group.keywords())) > MAX_KEYWORDS_BYTES || control(group.keywords()))
                return at + "的搜索关键词太长了。";
            List<String> items = groupItems(group.items());
            if (items.isEmpty()) return at + "还没有符号。";
            if (items.size() > MAX_GROUP_ITEMS) return at + "最多 " + MAX_GROUP_ITEMS + " 项。";
            total += items.size();
            Set<String> seen = new HashSet<>();
            for (String item : items) {
                if (item.length() > MAX_ITEM_UTF16 || control(item)) return at + "的「" + item + "」太长了。";
                if (!seen.add(item)) return at + "里「" + item + "」重复了。";
            }
        }
        if (count == 0) return "至少要有一组符号。";
        if (count > MAX_GROUPS) return "一个符号集最多 " + MAX_GROUPS + " 组。";
        if (total > MAX_SYMBOL_ITEMS) return "一个符号集合计最多 " + MAX_SYMBOL_ITEMS + " 项。";
        return null;
    }

    private static String wordsIssue(List<DraftWord> rows) {
        Set<String> seen = new HashSet<>();
        int count = 0;
        for (int index = 0; index < rows.size(); index++) {
            DraftWord row = rows.get(index);
            if (isBlankWord(row)) continue;
            count++;
            String at = "第 " + (index + 1) + " 个单词";
            String word = trim(row.word());
            String phonetic = trim(row.phonetic());
            String gloss = trim(row.gloss());
            if (word.isEmpty()) return at + "还没有填写单词。";
            if (characters(word) > MAX_WORD_CHARS || control(word)) return at + "太长了。";
            if (word.startsWith("#")) return at + "不能以 # 开头。";
            if (characters(phonetic) > MAX_PHONETIC_CHARS || control(phonetic)) return at + "的音标太长了。";
            if (gloss.isEmpty()) return at + "还没有填写释义。";
            if (characters(gloss) > MAX_GLOSS_CHARS || control(gloss)) return at + "的释义太长了。";
            if (!seen.add(word)) return "「" + word + "」出现了不止一次。";
        }
        if (count == 0) return "至少要有一个单词。";
        if (count > MAX_WORDS) return "一本单词本最多 " + MAX_WORDS + " 个单词。";
        return null;
    }

    /** 批量粘贴的结果：能读出的行，以及第一行读不出的原因（全部读出时为 null）。 */
    public record BulkRows<T>(List<T> rows, String issue) {}

    /** 批量添加短语：每行「编码 文字」，编码和文字之间用空格或 Tab 隔开，文字里可以再有空格；空行跳过。 */
    public static BulkRows<Phrase> parsePhraseLines(String text) {
        List<Phrase> rows = new ArrayList<>();
        String[] lines = LINE_BREAK.split(text, -1);
        for (int index = 0; index < lines.length; index++) {
            String line = lines[index];
            if (blank(line)) continue;
            Matcher match = PHRASE_LINE.matcher(line);
            if (!match.matches()) return new BulkRows<>(rows, "第 " + (index + 1) + " 行要写成「编码 文字」，中间用空格隔开。");
            rows.add(new Phrase(match.group(1), match.group(2)));
        }
        return new BulkRows<>(rows, null);
    }

    /** 批量添加单词：每行「单词 Tab 释义」或「单词 Tab 音标 Tab 释义」，从表格复制过来就是这样；空行和 # 开头的行跳过，与单词本的词表文件同一种写法。 */
    public static BulkRows<DraftWord> parseWordLines(String text) {
        List<DraftWord> rows = new ArrayList<>();
        String[] lines = LINE_BREAK.split(text, -1);
        for (int index = 0; index < lines.length; index++) {
            String line = lines[index];
            if (blank(line) || line.startsWith("#")) continue;
            String[] columns = line.split("\t", -1);
            if (columns.length == 2) rows.add(new DraftWord(columns[0], "", columns[1]));
            else if (columns.length == 3) rows.add(new DraftWord(columns[0], columns[1], columns[2]));
            else return new BulkRows<>(rows, "第 " + (index + 1)
                + " 行要写成「单词、释义」两列或「单词、音标、释义」三列，列之间用 Tab 隔开。");
        }
        return new BulkRows<>(rows, null);
    }

    /** 把批量读出的行接到已有的行后面；已有的只剩一个空行时直接替换它。 */
    public static <T> List<T> appendRows(List<T> current, List<T> added, java.util.function.Predicate<T> isBlank) {
        List<T> rows = new ArrayList<>(current.size() == 1 && isBlank.test(current.get(0)) ? List.of() : current);
        rows.addAll(added);
        return rows;
    }

    /**
     * 刚保存的包详情页顶上的一句话：短语表和指令表还要打开「启用」，K 模式或 / 指令关着时也要打开，别的类型装上就能用。`enabled` 是这个表已经启用，`modeOn` 是对应的快捷模式开着。
     *
     * <p>与共享设置页的 `authoredSaveNote` 一一对应，只把桌面上的去处换成手机上的：快捷模式的开关在这一页下面的提醒里（手机的「输入」页没有「快捷模式」），K 模式和 / 指令从键盘功能面板的「本地输入」进入，不是 Shift+K。
     */
    public static String authoredSaveNote(String kind, boolean enabled, boolean modeOn) {
        switch (kind) {
            case PHRASE_TABLE:
                if (enabled) {
                    return modeOn ? "已保存，修改已经生效。"
                        : "已保存。快捷短语（K 模式）现在关着，在下面打开后才会出候选。";
                }
                return modeOn ? "已保存。打开下面的「启用」后，在键盘的「本地输入」里点「快捷短语」再输入编码就能用到这些短语。"
                    : "已保存。打开下面的「启用」，再打开快捷短语（K 模式），这些短语才会出候选。";
            case COMMAND_TABLE:
                if (enabled) {
                    return modeOn ? "已保存，修改已经生效。"
                        : "已保存。/ 指令现在关着，在下面打开后才能用。";
                }
                return modeOn ? "已保存。打开下面的「启用」后，按 / 再输入指令字母就能用。"
                    : "已保存。打开下面的「启用」，再打开 / 指令，这些指令才能用。";
            case SYMBOL_SET:
                return "已保存，符号面板里会显示这些符号。";
            case WORDBOOK:
                return "已保存，这本书出现在背单词的词书里。";
            default:
                return "已保存。";
        }
    }

    // ---- 文本度量，与 TypeScript 的写法一致 ----

    /** JavaScript 的 `String.prototype.trim()`：去掉首尾的 WhiteSpace 和 LineTerminator。 */
    public static String trim(String value) {
        if (value == null) return "";
        int start = 0;
        int end = value.length();
        while (start < end && jsSpace(value.charAt(start))) start++;
        while (end > start && jsSpace(value.charAt(end - 1))) end--;
        return value.substring(start, end);
    }

    private static boolean jsSpace(char value) {
        return value == '\t' || value == '\n' || value == 0x0B || value == '\f' || value == '\r' || value == ' '
            || value == 0x00A0 || value == 0x1680 || (value >= 0x2000 && value <= 0x200A) || value == 0x2028
            || value == 0x2029 || value == 0x202F || value == 0x205F || value == 0x3000 || value == 0xFEFF;
    }

    public static boolean blank(String value) {
        return trim(value).isEmpty();
    }

    private static String optional(String value) {
        return blank(value) ? null : trim(value);
    }

    private static String orEmpty(String value) {
        return value == null ? "" : value;
    }

    private static int bytes(String value) {
        return value.getBytes(StandardCharsets.UTF_8).length;
    }

    private static int characters(String value) {
        return value.codePointCount(0, value.length());
    }

    /** `/\p{Cc}/u`：含有 C0、DEL 或 C1 控制字符。 */
    private static boolean control(String value) {
        if (value == null) return false;
        for (int index = 0; index < value.length(); index++) {
            if (Character.getType(value.charAt(index)) == Character.CONTROL) return true;
        }
        return false;
    }
}
