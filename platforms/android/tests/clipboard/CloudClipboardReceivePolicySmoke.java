package app.msime.android.clipboard;

import java.time.Instant;

public final class CloudClipboardReceivePolicySmoke {
    private static final String EARLY = "2026-11-13T00:00:01Z";
    private static final String LATE = "2026-11-13T00:00:02Z";
    private static final long EARLY_MS = Instant.parse(EARLY).toEpochMilli();

    public static void main(String[] args) {
        CloudClipboardReceivePolicy policy = new CloudClipboardReceivePolicy();
        check(policy.receive(policy.generation(), "synthetic remote", LATE, "synthetic old", EARLY_MS, true),
            "较新的远端复制可直接替换旧本机文本");
        check(!policy.localChanged("synthetic remote"), "接收回调不再上传");
        check(!policy.localChanged("synthetic remote"), "系统重复通知不形成循环");
        check(policy.localChanged("synthetic local"), "用户的新复制继续上传");
        check(!policy.receive(policy.generation(), "synthetic old", EARLY, null, 0, true),
            "删除云端最新项不能回放较旧的历史");

        CloudClipboardReceivePolicy raced = new CloudClipboardReceivePolicy();
        long request = raced.generation();
        raced.localChanged("synthetic concurrent");
        check(!raced.receive(request, "synthetic in flight", LATE, null, 0, true),
            "查询期间发生复制时旧请求不能写回");
        check(!raced.receive(raced.generation(), "synthetic in flight", LATE, null, 0, true),
            "下一轮不能重放已经因本机复制而放弃的修订");

        for (boolean replaceable : new boolean[] {true, false}) {
            CloudClipboardReceivePolicy blocked = new CloudClipboardReceivePolicy();
            check(!blocked.receive(blocked.generation(), "synthetic remote", EARLY,
                    "synthetic newer", EARLY_MS + 1, replaceable), "较新的本机复制始终优先");
        }
        CloudClipboardReceivePolicy opaque = new CloudClipboardReceivePolicy();
        check(!opaque.receive(opaque.generation(), "synthetic remote", LATE, null, 0, false),
            "敏感文本、图片或无法读取的剪贴板不能覆盖");
        CloudClipboardReceivePolicy unknown = new CloudClipboardReceivePolicy();
        check(!unknown.receive(unknown.generation(), "synthetic remote", LATE, "synthetic unknown", 0, true),
            "本机非空文本缺时间戳时不能推测云端更新");
        CloudClipboardReceivePolicy same = new CloudClipboardReceivePolicy();
        check(!same.receive(same.generation(), "synthetic same", LATE, "synthetic same", EARLY_MS, true),
            "文本相同不改写剪贴板");
        CloudClipboardReceivePolicy stopped = new CloudClipboardReceivePolicy();
        long active = stopped.generation();
        stopped.invalidate();
        check(!stopped.receive(active, "synthetic stopped", LATE, null, 0, true),
            "收起键盘、关闭同步和换绑后旧结果不得写回");
        CloudClipboardReceivePolicy precise = new CloudClipboardReceivePolicy();
        check(precise.receive(precise.generation(), "synthetic first", EARLY, null, 0, true), "空剪贴板可接收");
        check(precise.receive(precise.generation(), "synthetic second", "2026-11-13T00:00:01.000001Z",
                "synthetic first", EARLY_MS, true), "同一毫秒内的不同云端修订不能丢失");
        check(!precise.receive(precise.generation(), "synthetic invalid", "invalid", null, 0, true),
            "非法时间戳不能触发接收");
        check(!precise.receive(precise.generation(), "\u0000", LATE, null, 0, true), "非法文本不能触发接收");
        System.out.println("CloudClipboardReceivePolicySmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
