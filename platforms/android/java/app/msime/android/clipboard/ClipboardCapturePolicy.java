package app.msime.android;

/**
 * 系统剪贴板里的这一条要不要记进剪贴板历史。
 *
 * <p>记录有两个入口：复制时的监听（{@link Trigger#COPIED}），和打开面板时的补读（{@link Trigger#PANEL_OPENED}，补上键盘进程不在时复制的那一条）。补读原来不问这一条是不是已经处理过，于是用户清空历史、或者删掉一条之后，再打开面板，系统剪贴板里还留着的那一条又被记了回来，看上去就是「清空不生效」（#5605）。
 *
 * <p>所以每一条剪贴板内容都有一个身份：系统给的复制时刻（`ClipDescription.getTimestamp()`）加文字的散列。监听收到的复制一律记录——那是用户刚做的动作，哪怕内容和上一次一样；补读只记录还没处理过的那一条。身份只用来比较，不保存文字本身。
 */
public final class ClipboardCapturePolicy {
    /** 这次记录从哪里来。 */
    public enum Trigger {
        /** 系统通知剪贴板变了：用户刚复制。 */
        COPIED,
        /** 打开剪贴板面板时补读当前的系统剪贴板。 */
        PANEL_OPENED
    }

    private ClipboardCapturePolicy() {}

    /**
     * 一条剪贴板内容的身份。复制时刻读不到（小于等于 0）时只剩文字散列，同样的文字再复制一次也视为同一条，这时补读不会重复记录，复制时的监听仍会记录。
     */
    public static String identity(long copiedAtMs, String text) {
        if (text == null) throw new IllegalArgumentException("No clipboard text");
        long timestamp = Math.max(0, copiedAtMs);
        return timestamp + ":" + Integer.toHexString(text.hashCode()) + ":" + text.length();
    }

    /** 是否把这一条交给共享存储；`handledIdentity` 是上一次处理过的那一条，没有时为 null。 */
    public static boolean captures(Trigger trigger, String identity, String handledIdentity) {
        if (trigger == null || identity == null) return false;
        if (trigger == Trigger.COPIED) return true;
        return !identity.equals(handledIdentity);
    }
}
