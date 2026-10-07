import app.msime.android.HostDeepLink;

/** 宿主深链的契约：extras 名字、flags、tab 取值，以及对外来页面名和参数的过滤。 */
public final class HostDeepLinkSmoke {
    public static void main(String[] args) {
        // 这几个名字是跨进程契约：键盘进程、旧 Activity 跳板和设备测试都按字面值发。
        check("app.msime.android.home.HomeActivity".equals(HostDeepLink.HOME_ACTIVITY), "target is the launcher activity");
        check("msime_open_tab".equals(HostDeepLink.EXTRA_TAB), "tab extra name");
        check("msime_open_page".equals(HostDeepLink.EXTRA_PAGE), "page extra name");
        check("msime_page_args".equals(HostDeepLink.EXTRA_ARGS), "args extra name");

        // FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_CLEAR_TOP | FLAG_ACTIVITY_SINGLE_TOP
        check(HostDeepLink.FLAGS == (0x10000000 | 0x04000000 | 0x20000000), "flags are new task, clear top and single top");

        check(HostDeepLink.TAB_SETTINGS == 0 && HostDeepLink.TAB_COMMUNITY == 1
            && HostDeepLink.TAB_STATISTICS == 2 && HostDeepLink.TAB_ACCOUNT == 3, "tab indices follow the bottom bar");
        for (int tab = 0; tab <= 3; tab++) check(HostDeepLink.isTab(tab), "tab " + tab + " is accepted");
        check(!HostDeepLink.isTab(-1) && !HostDeepLink.isTab(4) && !HostDeepLink.isTab(Integer.MAX_VALUE), "out-of-range tabs are rejected");
        check(!HostDeepLink.isTab(HostDeepLink.NO_TAB), "the missing-tab marker is not a tab");

        // 页面名只认枚举名的形状；类名、路径、小写和超长输入都不算。
        check("LEXICON".equals(HostDeepLink.pageName("LEXICON")), "an enum name passes");
        check("LEXICON_DETAIL".equals(HostDeepLink.pageName("LEXICON_DETAIL")), "underscores pass");
        check("AI_SKIN".equals(HostDeepLink.pageName("AI_SKIN")), "letters after an underscore pass");
        check(HostDeepLink.pageName(null) == null, "a missing page is no page");
        check(HostDeepLink.pageName("") == null, "an empty page is no page");
        check(HostDeepLink.pageName("lexicon") == null, "lower case is not an enum name");
        check(HostDeepLink.pageName("app.msime.android.home.LexiconPage") == null, "a class name is never accepted");
        check(HostDeepLink.pageName("_LEXICON") == null, "a leading underscore is rejected");
        check(HostDeepLink.pageName("1LEXICON") == null, "a leading digit is rejected");
        check(HostDeepLink.pageName("LEXICON ") == null, "trailing whitespace is rejected");
        check(HostDeepLink.pageName("LEXICON\n") == null, "a trailing newline is rejected");
        check(HostDeepLink.pageName("A".repeat(HostDeepLink.MAX_PAGE_NAME)) != null, "a name at the limit passes");
        check(HostDeepLink.pageName("A".repeat(HostDeepLink.MAX_PAGE_NAME + 1)) == null, "a name over the limit is rejected");

        // 参数键名。
        check(HostDeepLink.isAllowedArgKey("add_language"), "a snake_case key passes");
        check(HostDeepLink.isAllowedArgKey("dictionary_id2"), "digits after the first letter pass");
        check(!HostDeepLink.isAllowedArgKey(null), "a null key is rejected");
        check(!HostDeepLink.isAllowedArgKey(""), "an empty key is rejected");
        check(!HostDeepLink.isAllowedArgKey("AddLanguage"), "upper case keys are rejected");
        check(!HostDeepLink.isAllowedArgKey("android.intent.extra.TEXT"), "dotted framework keys are rejected");
        check(!HostDeepLink.isAllowedArgKey("a".repeat(33)), "keys over 32 characters are rejected");
        check(HostDeepLink.isAllowedArgKey("a".repeat(32)), "a key at the limit passes");
        check(!HostDeepLink.isAllowedArgKey(HostDeepLink.ARG_EXTERNAL), "a sender cannot forge the external marker");

        // 参数值：只有 String / Boolean / Integer。
        check(HostDeepLink.isAllowedArgValue("pinyin"), "a short string passes");
        check(HostDeepLink.isAllowedArgValue(""), "an empty string passes");
        check(HostDeepLink.isAllowedArgValue(Boolean.TRUE), "a boolean passes");
        check(HostDeepLink.isAllowedArgValue(42), "an int passes");
        check(HostDeepLink.isAllowedArgValue("x".repeat(HostDeepLink.MAX_STRING_ARG)), "a string at the limit passes");
        check(!HostDeepLink.isAllowedArgValue("x".repeat(HostDeepLink.MAX_STRING_ARG + 1)), "a long string is rejected");
        check(!HostDeepLink.isAllowedArgValue(null), "null is rejected");
        check(!HostDeepLink.isAllowedArgValue(42L), "a long is rejected");
        check(!HostDeepLink.isAllowedArgValue(1.5), "a double is rejected");
        check(!HostDeepLink.isAllowedArgValue(new String[] {"a"}), "an array is rejected");
        check(!HostDeepLink.isAllowedArgValue(new StringBuilder("a")), "a non-String CharSequence is rejected");

        System.out.println("Android host deep link: extras, flags, tabs, page names and argument filter passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
