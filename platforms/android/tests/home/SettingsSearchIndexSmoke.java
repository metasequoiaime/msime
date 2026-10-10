import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 设置首页搜索的索引（`PageId` 每页的关键词）要覆盖详情页里的每一行设置。
 *
 * <p>关键词是手写的一张表，页面加了新行而忘了补关键词时，设置首页就搜不到它：#6132 里键盘页「布局」组的「浮动键盘」开关搜「浮动」显示「没有匹配的设置」，同一页的横屏分离式键盘、键盘底栏、按键动画等八行也一样。`PageId` 引用了 androidx 注解，进不了 check-host 只有 android.jar 的 JVM 编译，所以这里像 AndroidLocalSettingsSmoke 读 Rust 源码那样直接读 Java 源码：从 `PageId.java` 取出每页的标题与关键词，从对应页面类里取出开关、导航和滑块的字面量行标题，逐个按 `PageId#matches` 的规则（忽略大小写的包含）核对。按钮行是操作、值行是说明，不要求进索引；标题等于另一个可搜索页面标题的导航行是去那一页的入口，那一页自己能被搜到。行标题不是字面量的（共享的 InputFeatureToggle、语言表、工具栏按钮表）在 {@link #computedRows} 里逐条列出。
 */
public final class SettingsSearchIndexSmoke {
    private static final String HOME = "platforms/android/java/app/msime/android/home/";
    private static final String PAGE_ID = HOME + "PageId.java";

    /** `PageId` 的一项：`NAME("SimpleName", "标题", HostDeepLink.TAB_X, "关键词", ...)`。 */
    private static final Pattern PAGE = Pattern.compile(
        "^    ([A-Z_]+)\\(\"([A-Za-z]+)\", \"([^\"]*)\", HostDeepLink\\.TAB_[A-Z]+((?:,\\s*\"[^\"]*\")*)\\s*\\)[,;]",
        Pattern.MULTILINE);
    /** 独立数一遍 `PageId` 有几项，防止上面的正则漏掉某一项而让检查静悄悄地变少。 */
    private static final Pattern PAGE_HEAD = Pattern.compile("^    [A-Z_]+\\(\"[A-Za-z]+Page\", ", Pattern.MULTILINE);
    private static final Pattern QUOTED = Pattern.compile("\"([^\"]*)\"");
    /** 页面里的设置行：`GroupCard` 的 toggle / nav / slider，以及开发者页包了一层的 uploadToggle。 */
    private static final Pattern ROW = Pattern.compile("\\.(?:toggle|nav|slider)\\(\\s*\"([^\"]+)\"");
    private static final Pattern UPLOAD_ROW = Pattern.compile("\\buploadToggle\\(\\w+,\\s*\"([^\"]+)\"");

    private record Page(String name, String className, String title, List<String> keywords) {
        boolean searchable() { return !keywords.isEmpty(); }

        /** 与 `PageId#matches` 相同：去掉首尾空白、按 Locale.ROOT 转小写后，标题或任一关键词包含查询。 */
        boolean matches(String query) {
            String needle = query.trim().toLowerCase(Locale.ROOT);
            if (needle.isEmpty()) return false;
            if (title.toLowerCase(Locale.ROOT).contains(needle)) return true;
            for (String keyword : keywords) {
                if (keyword.toLowerCase(Locale.ROOT).contains(needle)) return true;
            }
            return false;
        }
    }

    public static void main(String[] args) throws IOException {
        Path root = repoRoot();
        Map<String, Page> pages = pages(root);
        List<String> missing = new ArrayList<>();
        literalRows(root, pages, missing);
        computedRows(pages, missing);
        if (!missing.isEmpty()) {
            throw new AssertionError("settings search index misses these rows; add them to the page's keywords in "
                + PAGE_ID + ":\n  " + String.join("\n  ", missing));
        }
        // #6132：搜「浮动」要找到键盘页。
        check(pages.get("KEYBOARD_OPTIONS").matches("浮动"), "searching 浮动 finds the keyboard page");
        System.out.println("SettingsSearchIndexSmoke ok");
    }

    private static Map<String, Page> pages(Path root) throws IOException {
        String source = read(root.resolve(PAGE_ID));
        Map<String, Page> pages = new LinkedHashMap<>();
        Matcher matcher = PAGE.matcher(source);
        while (matcher.find()) {
            List<String> keywords = new ArrayList<>();
            Matcher quoted = QUOTED.matcher(matcher.group(4));
            while (quoted.find()) keywords.add(quoted.group(1));
            pages.put(matcher.group(1), new Page(matcher.group(1), matcher.group(2), matcher.group(3), keywords));
        }
        int declared = 0;
        Matcher head = PAGE_HEAD.matcher(source);
        while (head.find()) declared++;
        check(declared > 0 && pages.size() == declared,
            "parsed " + pages.size() + " PageId entries but " + PAGE_ID + " declares " + declared);
        return pages;
    }

    /** 每个可搜索页面里字面量标题的设置行都要能用自己的标题搜到所在的页面。 */
    private static void literalRows(Path root, Map<String, Page> pages, List<String> missing) throws IOException {
        int checked = 0;
        for (Page page : pages.values()) {
            if (!page.searchable()) continue;
            Path file = root.resolve(HOME + page.className() + ".java");
            check(Files.isRegularFile(file), "page source exists for " + page.name() + ": " + file);
            String source = read(file);
            for (Pattern pattern : new Pattern[] {ROW, UPLOAD_ROW}) {
                Matcher matcher = pattern.matcher(source);
                while (matcher.find()) {
                    String row = matcher.group(1);
                    checked++;
                    if (linksToOtherPage(pages, page, row) || page.matches(row)) continue;
                    missing.add(page.name() + " (" + page.className() + "): " + row);
                }
            }
        }
        check(checked > 50, "found only " + checked + " setting rows; the row pattern no longer matches the pages");
    }

    /** 标题等于另一个可搜索页面标题的行（例如键盘页的「AI 润色与回复」）只是去那一页的入口。 */
    private static boolean linksToOtherPage(Map<String, Page> pages, Page owner, String row) {
        for (Page page : pages.values()) {
            if (page != owner && page.searchable() && page.title().equals(row)) return true;
        }
        return false;
    }

    /** 行标题来自代码里其他表的设置行：InputFeatureToggle 的标题、输入页的语言表、键盘页的工具栏按钮表。 */
    private static void computedRows(Map<String, Page> pages, List<String> missing) {
        String[][] rows = {
            {"TYPING", "只出单字"}, {"TYPING", "云候选"},
            {"TYPING", "普通话"}, {"TYPING", "粤语"}, {"TYPING", "日语"}, {"TYPING", "韩语"}, {"TYPING", "越南语"},
            {"TYPING", "藏语"},
            {"EXPRESSION", "智能标点"}, {"EXPRESSION", "英文联想"},
            {"LEXICON", "记忆新词"},
            {"PRIVACY", "用水杉账号翻译候选"}, {"PRIVACY", "匿名使用统计"}, {"PRIVACY", "剪贴板历史"},
            {"KEYBOARD_OPTIONS", "表情"}, {"KEYBOARD_OPTIONS", "常用语"}, {"KEYBOARD_OPTIONS", "剪贴板"},
            {"KEYBOARD_OPTIONS", "皮肤"}, {"KEYBOARD_OPTIONS", "输入方式"}, {"KEYBOARD_OPTIONS", "浮动键盘"},
            {"KEYBOARD_OPTIONS", "文本编辑"},
        };
        for (String[] row : rows) {
            Page page = pages.get(row[0]);
            check(page != null, "PageId has " + row[0]);
            if (!page.matches(row[1])) missing.add(page.name() + " (" + page.className() + "): " + row[1]);
        }
    }

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(PAGE_ID))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + PAGE_ID + " not found above user.dir");
    }

    private static String read(Path file) throws IOException {
        return new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
