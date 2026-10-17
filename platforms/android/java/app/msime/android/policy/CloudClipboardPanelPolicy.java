package app.msime.android;

import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.clipboard.CloudClipboardTextPolicy;

/**
 * What the keyboard's clipboard panel may do with the account's cloud clipboard, decided without touching a view or the network.
 *
 * <p>The cloud list is fetched when the panel opens and when the user asks for a refresh; nothing here reads the system clipboard or uploads on its own. Uploading is the user's explicit 「发到云剪贴板」 on a local entry, and only once the service has said, in this panel's own fetch, that this account is signed in and has the cloud clipboard turned on.
 *
 * <p>条目端到端加密，读取和上传都经共享驱动在本机解密、加密（{@link CloudClipboardE2e}）。这台设备还没被已有设备允许时（{@link Status#PENDING}），键盘只显示一句提示，不列条目，也不上传。
 *
 * <p>偏好里配置了自建中继时，「云端」读写的是中继（{@link CloudClipboardRelay}），不需要登录水杉账号；中继拒绝令牌或配置不全是 {@link Status#RELAY_REJECTED}，不是 {@link Status#SIGNED_OUT}，不能提示用户去登录。
 */
public final class CloudClipboardPanelPolicy {
    /** The two halves of the panel: this device's history and the account's cloud list. */
    public enum Tab { LOCAL, CLOUD }

    /** Where the cloud half stands after the latest fetch for the current field. */
    public enum Status { LOADING, SIGNED_OUT, DISABLED, PENDING, RELAY_REJECTED, FAILED, EMPTY, READY }

    public static final String TAB_LOCAL = "本机";
    public static final String TAB_CLOUD = "云端";
    public static final String UPLOAD_ACTION = "发到云剪贴板";
    public static final String SIGNED_OUT_MESSAGE = "登录水杉账号后可在设备间同步剪贴板";
    public static final String DISABLED_MESSAGE = "云剪贴板未开启";
    public static final String PENDING_MESSAGE = "这台设备等待允许，请在已同步的设备上打开云剪贴板设置允许";
    public static final String RELAY_REJECTED_MESSAGE = "中继拒绝了令牌或设置不完整，请到「我的 → 云剪贴板」检查";
    /** 驱动的错误码：云剪贴板关着，或本机还在等批准。 */
    private static final String CODE_DISABLED = "clipboard_sync_disabled";
    private static final String CODE_PENDING = "clipboard_device_pending";

    private static final int HTTP_UNAUTHORIZED = 401;
    private static final int HTTP_FORBIDDEN = 403;

    private CloudClipboardPanelPolicy() {}

    /**
     * Whether cloud entries may be listed in, inserted into, or sent from this field.
     *
     * <p>The same two signals that keep this keyboard from learning or counting key presses: a password field, and a field that asked for no personalised learning (an incognito tab, a private field). Text synced from another device is never offered there, and the upload action is withheld there too.
     */
    public static boolean cloudAllowed(int inputType, boolean allowLearning) {
        return allowLearning && !EditorPolicy.password(inputType);
    }

    /** The panel opens when either half has something to show in this field. */
    public static boolean panelAvailable(boolean localEnabled, boolean cloudAllowed) {
        return localEnabled || cloudAllowed;
    }

    /**
     * Which half the panel opens on.
     *
     * <p>用户上次选的那一半，只要这里还能用：敏感输入框一律回到本机历史。本机历史关着时也先停在本机，那里有「开启剪贴板历史」按钮；原来这时直接打开云端，没登录的用户只看到一句登录提示，以为剪贴板调不出来。
     */
    public static Tab initialTab(Tab previous, boolean cloudAllowed) {
        if (!cloudAllowed) return Tab.LOCAL;
        return previous == Tab.CLOUD ? Tab.CLOUD : Tab.LOCAL;
    }

    /** The answer of one successful fetch. Items from a disabled account are not shown. */
    public static Status loaded(boolean enabled, int itemCount) {
        return loaded(enabled, false, itemCount);
    }

    /** 同上；`pending` 为真表示这台设备还在等已有设备允许，这时没有条目可列。 */
    public static Status loaded(boolean enabled, boolean pending, int itemCount) {
        if (!enabled) return Status.DISABLED;
        if (pending) return Status.PENDING;
        return itemCount == 0 ? Status.EMPTY : Status.READY;
    }

    /** A failed fetch or upload: an HTTP 401/403 means the session is gone, anything else is worth a retry. */
    public static Status failed(int httpStatus) {
        return httpStatus == HTTP_UNAUTHORIZED || httpStatus == HTTP_FORBIDDEN
            ? Status.SIGNED_OUT : Status.FAILED;
    }

    /** 同上，先看驱动给的错误码：云剪贴板关着（服务端回 403）不是登录没了，本机待批准也不是失败，中继拒绝令牌也不是水杉账号没登录。 */
    public static Status failed(int httpStatus, String code) {
        if (CODE_DISABLED.equals(code)) return Status.DISABLED;
        if (CODE_PENDING.equals(code)) return Status.PENDING;
        if (CloudClipboardRelayPolicy.CODE_UNAUTHORIZED.equals(code)
                || CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED.equals(code)) return Status.RELAY_REJECTED;
        return failed(httpStatus);
    }

    /** 这个状态有专门的一句话（未登录、未开启、等待允许、中继被拒），上传被拒时就说这句，而不是笼统的「请稍后重试」。 */
    public static boolean explains(Status status) {
        return status == Status.SIGNED_OUT || status == Status.DISABLED || status == Status.PENDING
            || status == Status.RELAY_REJECTED;
    }

    /** Whether a fetch started for one field may still be drawn: the field and the panel opening it served are unchanged. */
    public static boolean accepts(long requestGeneration, long currentGeneration) {
        return requestGeneration == currentGeneration;
    }

    /** A cloud result or insertion is valid only while the same non-empty account owns the panel. */
    public static boolean acceptsAccount(String requestedAccount, String currentAccount) {
        return requestedAccount != null && !requestedAccount.isEmpty()
            && requestedAccount.equals(currentAccount);
    }

    /** A cloud result belongs to the same account binding, including sign-out and re-login lineage. */
    public static boolean acceptsBinding(String requestedAccount, long requestedGeneration,
            String currentAccount, long currentGeneration) {
        return requestedGeneration >= 0L && currentGeneration >= 0L
            && acceptsAccount(requestedAccount, currentAccount)
            && requestedGeneration == currentGeneration;
    }

    /** 云端条目只有在字段允许、页面已成功读取且账号绑定仍相同时才可使用。 */
    public static boolean canUseCloudRows(boolean cloudAllowed, Status status,
            String requestedAccount, long requestedGeneration,
            String currentAccount, long currentGeneration) {
        return cloudAllowed && status == Status.READY
            && acceptsBinding(requestedAccount, requestedGeneration,
                currentAccount, currentGeneration);
    }

    /** Whether an upload completion still belongs to the visible panel that started it. */
    public static boolean acceptsUploadResult(long requestGeneration, long currentGeneration) {
        return accepts(requestGeneration, currentGeneration);
    }

    /** Whether the cloud list should be drawn as entries rather than a single status line. */
    public static boolean showsItems(Status status) {
        return status == Status.READY;
    }

    /**
     * Whether 「发到云剪贴板」 may run for this local entry.
     *
     * <p>Only after this panel's fetch said the account is signed in with the cloud clipboard on (an empty or populated list), and only for text the service will accept.
     */
    public static boolean canUpload(boolean cloudAllowed, Status status, String text) {
        return cloudAllowed && (status == Status.EMPTY || status == Status.READY)
            && CloudClipboardTextPolicy.valid(text);
    }

    /** A failed reload invalidates the page before any mutating action may use its old rows. */
    public static boolean canMutate(boolean loaded, boolean busy) {
        return loaded && !busy;
    }

    /**
     * 云端分段在顶行显示的条数：只有这次拉取有了答复、账号开着云剪贴板（{@link Status#READY} 或 {@link Status#EMPTY}）时才有，读取中、未登录、未开启和失败时为 null，不显示。
     */
    public static Integer cloudCount(Status status, int itemCount) {
        return status == Status.READY || status == Status.EMPTY ? BoundsPolicy.nonNegative(itemCount) : null;
    }

    /** 这一分段的条数上限：本机历史由共享存储限定（{@link ClipboardHistoryPolicy#LIMIT}），云端由服务端限定（{@link CloudClipboardApi#MAX_ITEMS}）。 */
    public static int limit(Tab tab) {
        if (tab == null) throw new IllegalArgumentException("No clipboard tab");
        return tab == Tab.CLOUD ? CloudClipboardApi.MAX_ITEMS : ClipboardHistoryPolicy.DEFAULT_LIMIT;
    }

    public static int limit(Tab tab, int localLimit) {
        if (tab == null) throw new IllegalArgumentException("No clipboard tab");
        return tab == Tab.CLOUD ? CloudClipboardApi.MAX_ITEMS
            : ClipboardHistoryPolicy.isSupportedLimit(localLimit)
                ? localLimit : ClipboardHistoryPolicy.DEFAULT_LIMIT;
    }

    /**
     * 顶行「本机 / 云端」与「清空」之间居中的「已存条数/上限」（#5905），例如「10/50」。`count` 为 null 表示这一分段现在没有可数的内容（历史未开启、读取失败、云端不可用），返回空串，那块地方照旧留白。
     */
    public static String countLabel(Tab tab, Integer count) {
        if (count == null) return "";
        return count + "/" + limit(tab);
    }

    public static String countLabel(Tab tab, Integer count, int localLimit) {
        if (count == null) return "";
        return count + "/" + limit(tab, localLimit);
    }

    /** 同一个计数给读屏的说法，例如「本机已存 10 条，最多 50 条」；不显示时为空串。 */
    public static String countDescription(Tab tab, Integer count) {
        if (count == null) return "";
        return (tab == Tab.CLOUD ? TAB_CLOUD : TAB_LOCAL) + "已存 " + count + " 条，最多 " + limit(tab) + " 条";
    }

    public static String countDescription(Tab tab, Integer count, int localLimit) {
        if (count == null) return "";
        return (tab == Tab.CLOUD ? TAB_CLOUD : TAB_LOCAL) + "已存 " + count + " 条，最多 " + limit(tab, localLimit) + " 条";
    }

    /** The status line drawn above the cloud list. */
    public static String message(Status status, int itemCount) {
        if (status == null) throw new IllegalArgumentException("No cloud clipboard status");
        return switch (status) {
            case LOADING -> "正在读取云剪贴板…";
            case SIGNED_OUT -> SIGNED_OUT_MESSAGE;
            case DISABLED -> DISABLED_MESSAGE;
            case PENDING -> PENDING_MESSAGE;
            case RELAY_REJECTED -> RELAY_REJECTED_MESSAGE;
            case FAILED -> "云剪贴板读取失败，请刷新重试";
            case EMPTY -> "云剪贴板还没有内容 · 本机记录可通过管理发到云剪贴板";
            case READY -> itemCount + " 条 · 点按插入";
        };
    }
}
