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
        return receive(requestGeneration, text, updatedAt, localText, localCopiedAt, replaceable, false);
    }

    /** `ownWrite` 表示当前剪贴板是键盘接收后写入的：它的时间戳取自本机时钟写入那一刻，和服务器时间不可比，本轮已有云端水位时顺序只按水位判断。 */
    public boolean receive(long requestGeneration, String text, String updatedAt,
                           String localText, long localCopiedAt, boolean replaceable, boolean ownWrite) {
        if (!CloudClipboardTextPolicy.valid(text)) return false;
        final Instant remoteTime;
        try {
            remoteTime = Instant.parse(updatedAt);
        } catch (DateTimeParseException | ArithmeticException | NullPointerException invalid) {
            return false;
        }
        if (!remoteTime.isAfter(latestRemoteTime)) return false;
        // 服务重启后水位从零开始，自己写入的旧内容仍要按时间戳挡住被删除后回放的更旧修订。
        boolean ordered = ownWrite && latestRemoteTime.isAfter(Instant.EPOCH);
        latestRemoteTime = remoteTime;
        if (requestGeneration != generation || !replaceable || Objects.equals(text, localText)) return false;
        // 非空的本机文本没有可靠时间戳时保留它；系统时间戳用于拒绝旧云端内容。
        if (localText != null && !ordered && (localCopiedAt <= 0
                || !remoteTime.isAfter(Instant.ofEpochMilli(localCopiedAt)))) return false;
        echo = text;
        return true;
    }
}
