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
 * 底部导航的四格、`HomeActivity.TAB_IDS` 和 `HostDeepLink.TAB_*` 下标三处要对得上：0 设置、1 社区、2 统计、3 我的。
 *
 * <p>菜单和 `TAB_IDS` 引用了 `R`，进不了 check-host 只有 android.jar 的 JVM 编译，所以像 SettingsSearchIndexSmoke 那样直接读源码。更换入口时要同时改这三处，漏一处时深链会切到别的 tab，或者底部那一格点了没有页面。
 *
 * <p>社区那一格取代了原来的「全部设置」：它的根页是不带返回按钮的 `CommunityTabPage`，顶部固定「皮肤 / 词库」两个分类；打开皮肤或词库完整列表的入口都切到这一格，不再另压一份。皮肤仍是设置列表里的一行，归设置 tab。
 */
public final class BottomTabsSmoke {
    private static final String MENU = "platforms/android/res/menu/bottom_tabs.xml";
    private static final String HOME = "platforms/android/java/app/msime/android/home/HomeActivity.java";
    private static final String SETTINGS_LIST = "platforms/android/java/app/msime/android/home/KeyboardFragment.java";
    private static final String NAVIGATOR = "platforms/android/java/app/msime/android/home/SettingsNavigator.java";
    private static final String STRINGS = "platforms/android/res/values/strings.xml";
    private static final String PAGE_ID = "platforms/android/java/app/msime/android/home/PageId.java";
    private static final String COMMUNITY_ROOT = "platforms/android/java/app/msime/android/home/CommunityTabPage.java";
    private static final String COMMUNITY_LIST = "platforms/android/java/app/msime/android/home/CommunityFragment.java";
    private static final Pattern MENU_ITEM = Pattern.compile("<item android:id=\"@\\+id/(tab_[a-z]+)\"");
    private static final Pattern TAB_IDS = Pattern.compile("TAB_IDS = \\{([^}]*)\\}", Pattern.DOTALL);
    private static final Pattern TAB_ID = Pattern.compile("R\\.id\\.(tab_[a-z]+)");

    public static void main(String[] args) throws IOException {
        Path root = repoRoot();
        List<String> menu = matches(MENU_ITEM, read(root, MENU));
        String homeSource = read(root, HOME);
        Matcher literal = TAB_IDS.matcher(homeSource);
        check(literal.find(), "HomeActivity.TAB_IDS not found");
        List<String> home = matches(TAB_ID, literal.group(1));

        List<String> expected = new ArrayList<>(4);
        expected.add("tab_settings");
        expected.add("tab_community");
        expected.add("tab_statistics");
        expected.add("tab_account");
        check(menu.equals(expected), "bottom menu is 设置 / 社区 / 统计 / 我的, got " + menu);
        check(home.equals(expected), "HomeActivity.TAB_IDS follows the bottom menu, got " + home);

        check(expected.get(HostDeepLink.TAB_SETTINGS).equals("tab_settings"), "TAB_SETTINGS is the first item");
        check(expected.get(HostDeepLink.TAB_COMMUNITY).equals("tab_community"), "TAB_COMMUNITY is the second item");
        check(expected.get(HostDeepLink.TAB_STATISTICS).equals("tab_statistics"), "TAB_STATISTICS is the third item");
        check(expected.get(HostDeepLink.TAB_ACCOUNT).equals("tab_account"), "TAB_ACCOUNT is the fourth item");
        for (int tab = 0; tab < expected.size(); tab++) check(HostDeepLink.isTab(tab), "every bottom item has a deep-link index");
        check(!HostDeepLink.isTab(expected.size()), "no deep-link index past the last bottom item");

        // 第二格是社区：标题、图标和根页。
        String menuSource = read(root, MENU);
        String strings = read(root, STRINGS);
        check(menuSource.contains("<item android:id=\"@+id/tab_community\" android:icon=\"@drawable/ic_ms_public\" android:title=\"@string/tab_community\" />"),
            "the second bottom item uses the narrow globe icon and the tab_community title");
        check(strings.contains("<string name=\"tab_community\">社区</string>"), "tab_community is titled 社区");
        check(!strings.contains("name=\"tab_skins\"") && !strings.contains("name=\"tab_allsettings\""),
            "the skin and all-settings tab titles are gone");
        check(homeSource.contains("if (itemId == R.id.tab_community) {\n            CommunityTabPage page = new CommunityTabPage();")
                && homeSource.contains("return DetailPage.asTabRoot(page);"),
            "the community tab's root is CommunityTabPage hosted as a tab root, without a back button");
        String communityRoot = read(root, COMMUNITY_ROOT);
        check(communityRoot.contains("KINDS =\n        List.of(CommunityRequest.Kind.SKIN, CommunityRequest.Kind.DICTIONARY);")
                && communityRoot.contains("LABELS = List.of(\"皮肤\", \"词库\");")
                && communityRoot.contains("control.setOptions(LABELS, section);"),
            "the community tab root offers 皮肤 then 词库 as fixed top options");
        check(communityRoot.contains("CommunityFragment.embedded(KINDS.get(index))")
                && communityRoot.contains("state.putInt(STATE_SECTION, section);"),
            "each option embeds the full community list and the selection survives view recreation");
        check(!homeSource.contains("allSettings") && !read(root, SETTINGS_LIST).contains("all_settings"),
            "the all-settings tab mode is gone");

        // 皮肤回到设置列表，归设置 tab；打开它照常压栈。
        String settingsList = read(root, SETTINGS_LIST);
        check(settingsList.contains("row(appearance, PageId.SKINS, R.drawable.ic_ms_palette);")
                && settingsList.contains("case SKINS: return skin;"),
            "the settings list has the 皮肤 row with the current skin as its value");
        String pages = read(root, PAGE_ID);
        check(pages.contains("SKINS(\"SkinsPage\", \"皮肤\", HostDeepLink.TAB_SETTINGS,")
                && pages.contains("AI_SKIN(\"AiSkinPage\", \"AI 设计皮肤\", HostDeepLink.TAB_SETTINGS,"),
            "the skin pages belong to the settings tab");
        String navigator = read(root, NAVIGATOR);
        check(!navigator.contains("PageId.SKINS"), "opening the skin page pushes it like any other page");

        // 皮肤和词库的完整社区列表切到社区 tab 并选中那一类，其余类型照常压栈。
        check(read(root, COMMUNITY_LIST).contains("SettingsNavigator.open(context, PageId.COMMUNITY, arguments);"),
            "CommunityFragment.browse goes through SettingsNavigator");
        check(navigator.contains("CommunityRequest.Kind community = communityTabKind(page, args);\n        if (community != null) {\n            home.openCommunityTab(community);\n            return;\n        }")
                && navigator.contains("if (page != PageId.COMMUNITY) return null;")
                && navigator.contains("return CommunityTabPage.hosts(kind) ? kind : null;"),
            "browsing skins or dictionaries switches to the community tab instead of pushing a copy");
        check(homeSource.contains("int tab = page != null ? SettingsNavigator.tab(page, request.args) : request.tab;"),
            "a deep link to the skin or dictionary list selects the community tab first");
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
