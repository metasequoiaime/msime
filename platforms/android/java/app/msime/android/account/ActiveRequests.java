package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;

/**
 * 进行中的流式对话或 AI 皮肤生成的 request_id，退出登录时一次取消（#8201 迁移计划 5c）。
 *
 * <p>Rust 的流式对话只在回答完整以后才核对账号，生成中途退出登录时增量会继续交给页面，直到回答结束才报 account_cancelled；AI 皮肤一次生成从头到尾用开始时选定的账号。所以退出登录时由 Java 主动取消：{@link #cancelAll} 对每个还登记着的 request_id 调一次取消。取消本身交给构造时给的函数，冒烟测试换成记录调用的假函数。
 *
 * <p>纯 Java，不碰 org.json，可以在 JVM 冒烟里跑。
 */
final class ActiveRequests {
    private final Consumer<String> cancel;
    private final Set<String> active = new LinkedHashSet<>();

    ActiveRequests(Consumer<String> cancel) {
        this.cancel = cancel;
    }

    /** 一个新的 request_id（UUID 文本，36 个 [0-9a-f-]，符合头文件要求的 1–96 个 [A-Za-z0-9_-]），并登记为进行中。 */
    String begin() {
        String id = UUID.randomUUID().toString();
        synchronized (active) {
            active.add(id);
        }
        return id;
    }

    /** 这次调用结束了（成功、失败或已取消），不再需要在退出时取消。 */
    void end(String id) {
        synchronized (active) {
            active.remove(id);
        }
    }

    /** 还登记着的那几个。 */
    List<String> snapshot() {
        synchronized (active) {
            return new ArrayList<>(active);
        }
    }

    /** 退出登录：逐个取消还在进行的调用。取消函数抛出时跳过那一个，其余照常取消。 */
    void cancelAll() {
        for (String id : snapshot()) {
            try {
                cancel.accept(id);
            } catch (RuntimeException | LinkageError ignored) {
                // 取消只是尽早停下；停不了时那次调用结束后仍按账号核对丢掉结果。
            }
        }
    }
}
