package app.msime.android;

/**
 * 工具栏上的「最近复制」（#5692）：刚复制的一段文字在键盘顶部那一行显示一会儿，点一下就粘贴，省去打开剪贴板面板。
 *
 * <p>这里只管状态：哪一条、复制了多久、用户是不是已经用过或关掉了它。显示多久是 {@link #WINDOW_MS}；用过（点了粘贴）、关掉（点 ×）或开始打字之后，同一条不再出现，复制一条新的才会再出现。读系统剪贴板、隐私判断和画按钮都在服务里。
 */
public final class RecentClipboardSuggestion {
    /** 复制之后显示多久。 */
    public static final long WINDOW_MS = 60_000;
    /** 工具栏上最多显示多少个字（按 Unicode 码点），多出的用省略号。 */
    public static final int PREVIEW_CODE_POINTS = 40;

    private String identity;
    private String text;
    private long copiedAtMs;
    private String dismissedIdentity;

    /**
     * 工具栏是否提供「最近复制」。剪贴板历史关着时一律不提供，也不为它读系统剪贴板：用户关掉历史，就是不想让键盘接触复制的内容；历史开着时听本地开关 `platform.android.clipboard_suggestion`（默认开）。
     */
    public static boolean enabled(boolean clipboardHistoryEnabled, boolean suggestionSetting) {
        return clipboardHistoryEnabled && suggestionSetting;
    }

    /** 复制时刻是否还在显示窗口里；读不到复制时刻（小于等于 0）时为假，只有亲眼看到的复制才算刚复制。 */
    public static boolean fresh(long copiedAtMs, long nowMs) {
        return copiedAtMs > 0 && nowMs - copiedAtMs <= WINDOW_MS;
    }

    /**
     * 系统剪贴板里有一条刚复制的文字。用过或关掉的那一条（同一身份，见 {@link ClipboardCapturePolicy#identity}）不再提供。
     */
    public void offer(String identity, String text, long copiedAtMs) {
        if (identity == null || text == null || text.isBlank()) return;
        if (identity.equals(dismissedIdentity)) return;
        this.identity = identity;
        this.text = text;
        this.copiedAtMs = copiedAtMs;
    }

    /** 现在该显示的文字；没有、已经过了显示窗口时为 null。 */
    public String text(long nowMs) {
        if (text == null) return null;
        // 复制时刻比现在还晚（系统时钟被往回调过）时按刚复制处理，窗口仍从复制时刻算起。
        return nowMs - copiedAtMs <= WINDOW_MS ? text : null;
    }

    /** 离过期还有多久；没有可显示的内容时为 0。 */
    public long remainingMs(long nowMs) {
        if (text(nowMs) == null) return 0;
        return Math.max(0, copiedAtMs + WINDOW_MS - nowMs);
    }

    /** 用户用过或关掉了这一条：收起，同一条不再出现。 */
    public void dismiss() {
        if (identity != null) dismissedIdentity = identity;
        identity = null;
        text = null;
    }

    /** 工具栏上的预览：连续的空白（含换行）并成一个空格，去掉首尾空白，太长时截断加省略号。 */
    public static String preview(String text) {
        if (text == null) return "";
        StringBuilder out = new StringBuilder();
        boolean space = false;
        int codePoints = 0;
        String trimmed = text.strip();
        for (int offset = 0; offset < trimmed.length(); ) {
            int codePoint = trimmed.codePointAt(offset);
            offset += Character.charCount(codePoint);
            if (TextPolicy.isSpace(codePoint)) {
                space = true;
                continue;
            }
            boolean separated = space && out.length() > 0;
            if (codePoints + (separated ? 2 : 1) > PREVIEW_CODE_POINTS) {
                out.append('…');
                return out.toString();
            }
            if (separated) {
                out.append(' ');
                codePoints++;
            }
            space = false;
            out.appendCodePoint(codePoint);
            codePoints++;
        }
        return out.toString();
    }
}
