package app.msime.android;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.RejectedExecutionException;

public final class CloudClipboardAutoSyncSmoke {
    public static void main(String[] args) {
        ArrayDeque<Runnable> queue = new ArrayDeque<>();
        List<String> uploads = new ArrayList<>();
        String[] current = {"account-a"};
        CloudClipboardAutoSync.Destinations destinations = expected -> new CloudClipboardAutoSync.Destination() {
            public boolean ready() {
                require(expected.equals(current[0]), "排队期间换号时应拒绝旧任务");
                return true;
            }
            public void upload(String text) {
                require(expected.equals(current[0]), "状态查询后换号时应拒绝旧任务");
                uploads.add(expected + ":" + text);
            }
        };
        CloudClipboardAutoSync.enqueue(queue::add, current[0], "合成文本", destinations);
        require(uploads.isEmpty(), "监听不能同步执行网络任务");
        current[0] = "account-b";
        queue.remove().run();
        require(uploads.isEmpty(), "不能把账号 A 的内容上传到账号 B");
        CloudClipboardAutoSync.enqueue(queue::add, current[0], "合成文本", destinations);
        queue.remove().run();
        require(uploads.equals(List.of("account-b:合成文本")), "新复制应沿新的绑定正常上传一次");

        for (boolean ready : new boolean[]{false, true}) {
            int before = uploads.size();
            CloudClipboardAutoSync.enqueue(queue::add, "account-b", "合成文本", expected -> new CloudClipboardAutoSync.Destination() {
                public boolean ready() {
                    if (ready) current[0] = "account-c";
                    return ready;
                }
                public void upload(String text) {
                    require(expected.equals(current[0]), "查询和上传之间不能换号");
                    uploads.add(text);
                }
            });
            queue.remove().run();
            require(uploads.size() == before, "关闭、待批准或查询后换号均不上传");
        }
        CloudClipboardAutoSync.enqueue(queue::add, "account-a", "合成文本", expected -> {
            throw new IllegalStateException("合成的选路变化");
        });
        queue.remove().run();
        require(queue.isEmpty(), "失败不建立重试队列");
        CloudClipboardAutoSync.enqueue(task -> { throw new RejectedExecutionException(); },
            "account-a", "合成文本", destinations);
        System.out.println("CloudClipboardAutoSyncSmoke: passed");
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new IllegalStateException(message);
    }
}
