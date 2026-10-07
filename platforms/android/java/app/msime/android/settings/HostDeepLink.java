package app.msime.android;

import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import java.util.regex.Pattern;

/**
 * 打开宿主某个 tab 或某个设置页的深链：键盘进程、旧的二级 Activity 和宿主自己都经这里拼 Intent，HomeActivity 经这里读。
 *
 * <p>本类是纯 Java，不 import `androidx`、`home/` 或 `R`：`:ime` 进程里的 `MSIMEInputService` 也要用它，而 check-host 会编译并检查这个目录下的每个文件。目标 Activity 只能按类名指定，因为键盘这一侧不能引用 `home/` 的类。
 *
 * <p>HomeActivity 是 exported 的，任何应用都能发来这样的 Intent，所以读取时一律当作不可信输入：tab 只认 0–3；页面名只认形如 `LEXICON` 的大写标识符（HomeActivity 再用 `PageId.valueOf` 映射，不认识就忽略，绝不接受 Intent 给出的类名）；参数只保留键名合法、值为 String / boolean / int 的条目，且数量和长度有上限。参数只影响导航和初始滚动位置，页面不得据此触发联网、删除、注销、登录、上传这类动作，读进来的参数都带 {@link #ARG_EXTERNAL} 标记，页面可以据此区分。
 */
public final class HostDeepLink {
    /** 宿主主界面的全限定类名；`smoke.sh` 与各设备测试用的是同一个名字。 */
    public static final String HOME_ACTIVITY = "app.msime.android.home.HomeActivity";

    /** int：要打开的 tab，取值见下面四个常量。 */
    public static final String EXTRA_TAB = "msime_open_tab";
    /** String，可空：`PageId` 的枚举名。同时给了 tab 时以页面所属的 tab 为准。 */
    public static final String EXTRA_PAGE = "msime_open_page";
    /** Bundle，可空：交给页面的参数，只允许 String / boolean / int 值。 */
    public static final String EXTRA_ARGS = "msime_page_args";
    /** HomeActivity 给从 Intent 读进来的参数加上的标记，值为 true；页面看到它就只能把参数用于导航和滚动。 */
    public static final String ARG_EXTERNAL = "msime_external";

    public static final int TAB_SETTINGS = 0;
    public static final int TAB_COMMUNITY = 1;
    public static final int TAB_STATISTICS = 2;
    public static final int TAB_ACCOUNT = 3;
    /** 没有给 tab 或给了不认识的值。 */
    public static final int NO_TAB = -1;

    /** 键盘进程没有自己的任务栈，必须 NEW_TASK；CLEAR_TOP + SINGLE_TOP 让已经开着的宿主收到 `onNewIntent`，而不是再叠一个。 */
    public static final int FLAGS =
        Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP;

    /** 页面名长度上限；最长的枚举名远短于它，超过就一定不是我们自己发的。 */
    public static final int MAX_PAGE_NAME = 48;
    /** 参数条目数上限。 */
    public static final int MAX_ARGS = 8;
    /** String 参数的长度上限（UTF-16 单元）。 */
    public static final int MAX_STRING_ARG = 256;

    private static final Pattern PAGE_NAME = Pattern.compile("[A-Z][A-Z0-9_]*");
    private static final Pattern ARG_KEY = Pattern.compile("[a-z][a-z0-9_]{0,31}");

    private HostDeepLink() {}

    /** 打开某个 tab 的根页。 */
    public static Intent tab(Context context, int tab) {
        if (!isTab(tab)) throw new IllegalArgumentException("unknown tab " + tab);
        Intent intent = base(context);
        intent.putExtra(EXTRA_TAB, tab);
        return intent;
    }

    /**
     * 打开某个设置页，宿主会先切到它所属的 tab。
     *
     * @param page `PageId` 的枚举名，例如 `LEXICON`
     * @param args 可空；只能放 String / boolean / int，其他类型在宿主那一侧会被丢掉
     */
    public static Intent page(Context context, String page, Bundle args) {
        if (pageName(page) == null) throw new IllegalArgumentException("not a page name: " + page);
        Intent intent = base(context);
        intent.putExtra(EXTRA_PAGE, page);
        if (args != null) intent.putExtra(EXTRA_ARGS, args);
        return intent;
    }

    private static Intent base(Context context) {
        Intent intent = new Intent();
        intent.setClassName(context, HOME_ACTIVITY);
        intent.addFlags(FLAGS);
        return intent;
    }

    /** 宿主读到的请求；三个字段都可能是「没有」。 */
    public static final class Request {
        /** 要切到的 tab，没有时为 {@link #NO_TAB}。 */
        public final int tab;
        /** 语法合法的页面名，没有时为 null；是否真的是一个页面由宿主判断。 */
        public final String page;
        /** 过滤后的参数，带 {@link #ARG_EXTERNAL}；没有页面时为 null。 */
        public final Bundle args;

        Request(int tab, String page, Bundle args) {
            this.tab = tab;
            this.page = page;
            this.args = args;
        }

        /** 这个 Intent 里有没有任何要导航的东西。 */
        public boolean isEmpty() { return tab == NO_TAB && page == null; }
    }

    /**
     * 从发给宿主的 Intent 里读出导航请求；任何不合规的部分都被丢掉而不是报错。
     *
     * <p>这里是系统边界：extras 由别的应用构造，解包时可能因为对方塞进了我们不认识的 Parcelable 而抛 `BadParcelableException` 等运行时异常，这种 Intent 整个当作没有导航请求。
     */
    public static Request read(Intent intent) {
        if (intent == null) return new Request(NO_TAB, null, null);
        try {
            int tab = intent.getIntExtra(EXTRA_TAB, NO_TAB);
            if (!isTab(tab)) tab = NO_TAB;
            String page = pageName(intent.getStringExtra(EXTRA_PAGE));
            Bundle args = page == null ? null : sanitizeArgs(intent.getBundleExtra(EXTRA_ARGS));
            return new Request(tab, page, args);
        } catch (RuntimeException malformed) {
            return new Request(NO_TAB, null, null);
        }
    }

    /** 只保留键名合法、值类型允许的条目，并加上 {@link #ARG_EXTERNAL}。 */
    @SuppressWarnings("deprecation")
    static Bundle sanitizeArgs(Bundle raw) {
        Bundle clean = new Bundle();
        if (raw != null) {
            int kept = 0;
            for (String key : raw.keySet()) {
                if (kept >= MAX_ARGS) break;
                if (!isAllowedArgKey(key)) continue;
                // `BaseBundle.get` 在 API 33 弃用，但只有它能在不知道类型时取出值并按类型过滤；按类型逐个 getX 会把错类型的值静默读成默认值。
                Object value = raw.get(key);
                if (!isAllowedArgValue(value)) continue;
                if (value instanceof String text) clean.putString(key, text);
                else if (value instanceof Boolean flag) clean.putBoolean(key, flag);
                else clean.putInt(key, (Integer) value);
                kept++;
            }
        }
        clean.putBoolean(ARG_EXTERNAL, true);
        return clean;
    }

    public static boolean isTab(int tab) {
        return tab >= TAB_SETTINGS && tab <= TAB_ACCOUNT;
    }

    /** 语法上像一个 `PageId` 名字就原样返回，否则返回 null。 */
    public static String pageName(String raw) {
        if (raw == null || raw.isEmpty() || raw.length() > MAX_PAGE_NAME) return null;
        return PAGE_NAME.matcher(raw).matches() ? raw : null;
    }

    /** 参数键名：小写字母开头，只含小写字母、数字和下划线，至多 32 个字符；{@link #ARG_EXTERNAL} 由宿主自己加，外部给的一律丢掉。 */
    public static boolean isAllowedArgKey(String key) {
        return key != null && !ARG_EXTERNAL.equals(key) && ARG_KEY.matcher(key).matches();
    }

    /** 参数值：String（不超过 {@link #MAX_STRING_ARG}）、Boolean 或 Integer。 */
    public static boolean isAllowedArgValue(Object value) {
        if (value instanceof String text) return text.length() <= MAX_STRING_ARG;
        return value instanceof Boolean || value instanceof Integer;
    }
}
