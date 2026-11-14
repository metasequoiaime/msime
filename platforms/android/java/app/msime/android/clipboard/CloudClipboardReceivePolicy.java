package app.msime.android.clipboard;

import java.time.Instant;
import java.time.format.DateTimeParseException;
import java.util.Objects;

/** 自动接收只处理云端的新修订，不覆盖更新的本机复制，也不把接收回调再上传。 */
public final class CloudClipboardReceivePolicy {
    private long generation;
    private Instant latestRemoteTime = Instant.EPOCH;
    private String echo;

    public long generation() { return generation; }

    public void invalidate() {
        generation++;
        echo = null;
    }

    /** 返回这次变化是否来自本机；系统可以为同一次写入发送多次通知。 */
    public boolean localChanged(String text) {
        if (text != null && Objects.equals(text, echo)) return false;
        invalidate();
        return true;
    }

    public boolean receive(long requestGeneration, String text, String updatedAt,
                           String localText, long localCopiedAt, boolean replaceable) {
        if (!CloudClipboardTextPolicy.valid(text)) return false;
        final Instant remoteTime;
        try {
            remoteTime = Instant.parse(updatedAt);
        } catch (DateTimeParseException | ArithmeticException | NullPointerException invalid) {
            return false;
        }
        if (!remoteTime.isAfter(latestRemoteTime)) return false;
        latestRemoteTime = remoteTime;
        if (requestGeneration != generation || !replaceable || Objects.equals(text, localText)) return false;
        // 非空的本机文本没有可靠时间戳时保留它；系统时间戳用于拒绝旧云端内容。
        if (localText != null && (localCopiedAt <= 0
                || !remoteTime.isAfter(Instant.ofEpochMilli(localCopiedAt)))) return false;
        echo = text;
        return true;
    }
}
