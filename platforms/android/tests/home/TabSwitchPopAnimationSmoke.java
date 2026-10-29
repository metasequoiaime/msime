import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 详情页压在栈上时点另一个底部 tab，清栈那一次不播回退动画（#10092）。
 *
 * <p>回退栈记录带着详情页的淡出和下面 tab 根页的淡入。换 tab 前清栈时照常播的话，被弹回来的根页一边淡入一边等着被切 tab 藏起来，和目标 tab 的淡入叠成重影，例如「我的 → 皮肤」后点「首页」，标题叠成「我杉拼音」。修法是 `HomeActivity.selectTab` 只在这次弹栈期间打上标记，`home_content` 里的页面在 `onCreateAnimation` 里看到标记就换成零时长的透明动画。
 *
 * <p>Fragment 动画只能在设备上看，HomeActivity 和页面类又依赖 androidx，进不了 check-host 只有 android.jar 的 JVM 编译，所以像 BottomTabsSmoke 那样直接读源码：标记只包住换 tab 时的清栈，以及每个能进 `home_content` 的页面类（PageId 登记的页面和四个 tab 根页）都会走到这段判断。新加的页面直接继承 Fragment 而不继承 HomeTabFragment 时，这里会失败。
 */
public final class TabSwitchPopAnimationSmoke {
    private static final String HOME_DIR = "platforms/android/java/app/msime/android/home/";
    private static final String HOME = HOME_DIR + "HomeActivity.java";
    private static final String BASE = HOME_DIR + "HomeTabFragment.java";
    private static final String PAGE_ID = HOME_DIR + "PageId.java";
    private static final Pattern PAGE_ENTRY = Pattern.compile("(?m)^    [A-Z_]+\\(\"([A-Za-z]+)\"");
    private static final Pattern TAB_ROOT = Pattern.compile("new ([A-Za-z]+)\\(\\);");
    private static final String HOOK = "HomeTabFragment.leavingWithoutAnimation(this)";

    public static void main(String[] args) throws IOException {
        Path root = repoRoot();
        String home = read(root, HOME);

        String selectTab = body(home, "private void selectTab(int itemId) {");
        check(selectTab.contains("if (itemId != selected) {")
                && ordered(selectTab, "clearingForTabSwitch = true;", "clearStack();", "clearingForTabSwitch = false;", "show(itemId);"),
            "selectTab marks only the clear before a tab switch, and clears the mark before show() fades the target tab in");
        check(count(home, "clearingForTabSwitch = true;") == 1,
            "only switching tabs suppresses the pop animation; back and reselecting the current tab keep it");
        String clearStack = body(home, "private void clearStack() {");
        check(clearStack.contains("popBackStackImmediate("),
            "the clear is synchronous, so the mark covers exactly that one pop");

        String base = read(root, BASE);
        check(base.contains("@Override @Nullable public Animation onCreateAnimation(int transit, boolean enter, int nextAnim) {\n        Animation none = leavingWithoutAnimation(this);")
                && base.contains("return none != null ? none : super.onCreateAnimation(transit, enter, nextAnim);"),
            "HomeTabFragment replaces the back-stack animation only while the stack is cleared for a tab switch");
        String helper = body(base, "@Nullable static Animation leavingWithoutAnimation(Fragment page) {");
        check(helper.contains("home.clearingForTabSwitch()")
                && helper.contains("new AlphaAnimation(0f, 0f)")
                && helper.contains("none.setDuration(0);"),
            "the replacement is a zero-length transparent animation, not null (null falls back to the fade_through resources)");

        Set<String> pages = new LinkedHashSet<>();
        Matcher entry = PAGE_ENTRY.matcher(read(root, PAGE_ID));
        while (entry.find()) pages.add(entry.group(1));
        check(pages.size() > 30, "PageId entries not found, got " + pages);
        Matcher tabRoot = TAB_ROOT.matcher(body(home, "private Fragment create(int itemId) {"));
        List<String> roots = new ArrayList<>(4);
        while (tabRoot.find()) roots.add(tabRoot.group(1));
        check(roots.size() == 4, "HomeActivity.create builds four tab roots, got " + roots);
        pages.addAll(roots);

        for (String page : pages) {
            Path source = root.resolve(HOME_DIR + page + ".java");
            // PageId 允许页面类还没建（SettingsNavigator 会提示「即将推出」），那种页面进不了回退栈。
            if (!Files.isRegularFile(source)) continue;
            check(reachesBase(root, page) || read(root, HOME_DIR + page + ".java").contains(HOOK),
                page + " can sit in home_content but neither extends HomeTabFragment nor calls " + HOOK);
        }
        System.out.println("Android tab switch pop animation passed");
    }

    /** 沿 `extends` 往上找，能走到 HomeTabFragment 为真；走出 home 包（例如 androidx 的 Fragment）为假。 */
    private static boolean reachesBase(Path root, String simpleName) throws IOException {
        String current = simpleName;
        for (int depth = 0; depth < 10; depth++) {
            if (current.equals("HomeTabFragment")) return true;
            Path source = root.resolve(HOME_DIR + current + ".java");
            if (!Files.isRegularFile(source)) return false;
            Matcher parent = Pattern.compile("class " + current + "(?:<[^>]*>)? extends ([A-Za-z]+)")
                .matcher(new String(Files.readAllBytes(source), StandardCharsets.UTF_8));
            if (!parent.find()) return false;
            current = parent.group(1);
        }
        return false;
    }

    /** 从 `signature` 开始到与它的左花括号配对的右花括号为止。 */
    private static String body(String source, String signature) {
        int start = source.indexOf(signature);
        check(start >= 0, "not found: " + signature);
        int depth = 0;
        for (int i = start + signature.length() - 1; i < source.length(); i++) {
            char c = source.charAt(i);
            if (c == '{') depth++;
            else if (c == '}' && --depth == 0) return source.substring(start, i + 1);
        }
        throw new AssertionError("unbalanced braces after " + signature);
    }

    private static boolean ordered(String text, String... parts) {
        int from = 0;
        for (String part : parts) {
            int at = text.indexOf(part, from);
            if (at < 0) return false;
            from = at + part.length();
        }
        return true;
    }

    private static int count(String text, String part) {
        int found = 0;
        for (int at = text.indexOf(part); at >= 0; at = text.indexOf(part, at + part.length())) found++;
        return found;
    }

    private static String read(Path root, String relative) throws IOException {
        return new String(Files.readAllBytes(root.resolve(relative)), StandardCharsets.UTF_8);
    }

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(HOME))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + HOME + " not found above user.dir");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
