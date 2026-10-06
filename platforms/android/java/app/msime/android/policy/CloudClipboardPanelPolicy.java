package app.msime.android;

import app.msime.android.clipboard.CloudClipboardTextPolicy;

/**
 * What the keyboard's clipboard panel may do with the account's cloud clipboard, decided without touching a view or the network.
 *
 * <p>The cloud list is fetched when the panel opens and when the user asks for a refresh; nothing here reads the system clipboard or uploads on its own. Uploading is the user's explicit 「发到云剪贴板」 on a local entry, and only once the service has said, in this panel's own fetch, that this account is signed in and has the cloud clipboard turned on.
 */
public final class CloudClipboardPanelPolicy {
    /** The two halves of the panel: this device's history and the account's cloud list. */
    public enum Tab { LOCAL, CLOUD }

    /** Where the cloud half stands after the latest fetch for the current field. */
    public enum Status { LOADING, SIGNED_OUT, DISABLED, FAILED, EMPTY, READY }

    public static final String TAB_LOCAL = "本机";
    public static final String TAB_CLOUD = "云端";
    public static final String UPLOAD_ACTION = "发到云剪贴板";
    public static final String SIGNED_OUT_MESSAGE = "登录水杉账号后可在设备间同步剪贴板";
    public static final String DISABLED_MESSAGE = "云剪贴板未开启";

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
     * <p>The half the user last chose, as long as it is still usable here: a sensitive field always falls back to the local history, and with local history switched off the cloud half is the only one with content.
     */
    public static Tab initialTab(Tab previous, boolean localEnabled, boolean cloudAllowed) {
        if (!cloudAllowed) return Tab.LOCAL;
        if (!localEnabled) return Tab.CLOUD;
        return previous == Tab.CLOUD ? Tab.CLOUD : Tab.LOCAL;
    }

    /** The answer of one successful fetch. Items from a disabled account are not shown. */
    public static Status loaded(boolean enabled, int itemCount) {
        if (!enabled) return Status.DISABLED;
        return itemCount == 0 ? Status.EMPTY : Status.READY;
    }

    /** A failed fetch or upload: an HTTP 401/403 means the session is gone, anything else is worth a retry. */
    public static Status failed(int httpStatus) {
        return httpStatus == HTTP_UNAUTHORIZED || httpStatus == HTTP_FORBIDDEN
            ? Status.SIGNED_OUT : Status.FAILED;
    }

    /** Whether a fetch started for one field may still be drawn: the field and the panel opening it served are unchanged. */
    public static boolean accepts(long requestGeneration, long currentGeneration) {
        return requestGeneration == currentGeneration;
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

    /** The status line drawn above the cloud list. */
    public static String message(Status status, int itemCount) {
        if (status == null) throw new IllegalArgumentException("No cloud clipboard status");
        return switch (status) {
            case LOADING -> "正在读取云剪贴板…";
            case SIGNED_OUT -> SIGNED_OUT_MESSAGE;
            case DISABLED -> DISABLED_MESSAGE;
            case FAILED -> "云剪贴板读取失败，请刷新重试";
            case EMPTY -> "云剪贴板还没有内容 · 本机记录可通过管理发到云剪贴板";
            case READY -> itemCount + " 条 · 点按插入";
        };
    }
}
