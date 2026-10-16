import app.msime.android.HostDeepLink;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 底部导航的四格、`HomeActivity.TAB_IDS` 和 `HostDeepLink.TAB_*` 下标三处要对得上：0 设置、1 全部设置、2 统计、3 我的。
 *
 * <p>菜单和 `TAB_IDS` 引用了 `R`，进不了 check-host 只有 android.jar 的 JVM 编译，所以像 SettingsSearchIndexSmoke 那样直接读源码。更换入口时要同时改这三处，漏一处时深链会切到别的 tab，或者底部那一格点了没有页面。
 */
public final class BottomTabsSmoke {
    private static final String MENU = "platforms/android/res/menu/bottom_tabs.xml";
    private static final String HOME = "platforms/android/java/app/msime/android/home/HomeActivity.java";
    private static final Pattern MENU_ITEM = Pattern.compile("<item android:id=\"@\\+id/(tab_[a-z]+)\"");
    private static final Pattern TAB_IDS = Pattern.compile("TAB_IDS = \\{([^}]*)\\}", Pattern.DOTALL);
    private static final Pattern TAB_ID = Pattern.compile("R\\.id\\.(tab_[a-z]+)");

    public static void main(String[] args) throws IOException {
        Path root = repoRoot();
        List<String> menu = matches(MENU_ITEM, read(root, MENU));
        Matcher literal = TAB_IDS.matcher(read(root, HOME));
        check(literal.find(), "HomeActivity.TAB_IDS not found");
        List<String> home = matches(TAB_ID, literal.group(1));

        List<String> expected = new ArrayList<>(4);
        expected.add("tab_settings");
        expected.add("tab_allsettings");
        expected.add("tab_statistics");
        expected.add("tab_account");
        check(menu.equals(expected), "bottom menu is 设置 / 全部设置 / 统计 / 我的, got " + menu);
        check(home.equals(expected), "HomeActivity.TAB_IDS follows the bottom menu, got " + home);

        check(expected.get(HostDeepLink.TAB_SETTINGS).equals("tab_settings"), "TAB_SETTINGS is the first item");
        check(expected.get(HostDeepLink.TAB_ALL_SETTINGS).equals("tab_allsettings"), "TAB_ALL_SETTINGS is the second item");
        check(expected.get(HostDeepLink.TAB_STATISTICS).equals("tab_statistics"), "TAB_STATISTICS is the third item");
        check(expected.get(HostDeepLink.TAB_ACCOUNT).equals("tab_account"), "TAB_ACCOUNT is the fourth item");
        for (int tab = 0; tab < expected.size(); tab++) check(HostDeepLink.isTab(tab), "every bottom item has a deep-link index");
        check(!HostDeepLink.isTab(expected.size()), "no deep-link index past the last bottom item");
        System.out.println("Android bottom tabs passed");
    }

    private static List<String> matches(Pattern pattern, String text) {
        List<String> found = new ArrayList<>(4);
        Matcher matcher = pattern.matcher(text);
        while (matcher.find()) found.add(matcher.group(1));
        return found;
    }

    private static String read(Path root, String relative) throws IOException {
        return new String(Files.readAllBytes(root.resolve(relative)), StandardCharsets.UTF_8);
    }

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(MENU))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + MENU + " not found above user.dir");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
