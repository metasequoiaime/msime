package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/** 退出登录时取消还在进行的流式对话和 AI 皮肤生成（#8201 迁移计划 5c）：Rust 只在回答完整以后才核对账号，不主动取消的话增量会继续交给页面。 */
public final class ActiveRequestsSmoke {
    public static void main(String[] arguments) {
        List<String> cancelled = new ArrayList<>();
        ActiveRequests requests = new ActiveRequests(cancelled::add);
        String first = requests.begin();
        String second = requests.begin();
        check(!first.equals(second), "every call gets its own request id");
        check(first.matches("[A-Za-z0-9_-]{1,96}"), "a request id the header accepts: " + first);
        requests.end(first);
        requests.cancelAll();
        check(cancelled.equals(List.of(second)), "signing out cancels only what is still running");

        cancelled.clear();
        requests.end(second);
        requests.cancelAll();
        check(cancelled.isEmpty(), "nothing is cancelled once every call has ended");

        List<String> survivors = new ArrayList<>();
        ActiveRequests throwing = new ActiveRequests(id -> {
            if (survivors.isEmpty()) {
                survivors.add(id);
                throw new IllegalStateException("synthetic cancel failure");
            }
            survivors.add(id);
        });
        throwing.begin();
        throwing.begin();
        throwing.cancelAll();
        check(survivors.size() == 2, "a failing cancel does not stop the others");
        System.out.println("Android active request cancellation passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
