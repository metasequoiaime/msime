package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;

/**
 * 本机数据有改动时通知云同步，以及在任何进程里读同步开关。
 *
 * <p>主进程直接读写 {@link SyncSwitch}；`:ime` 键盘进程不碰那份存储，经 {@link AccountSessionProvider} 的 `sync_dirty` / `sync_state` 由主进程代为读写，和访问令牌的路由一样（{@link AccountSessionRoutingPolicy}）。provider 只回答开关与登录方式，绝不返回令牌。
 */
public final class SyncSignals {
    /** `:ime` 进程读到的同步状态。 */
    public record State(boolean enabled, String loginKind) {
        public static final State OFF = new State(false, "");
    }

    private SyncSignals() {}

    /**
     * 标记一个分类（{@link SyncSwitch#SECTIONS}）本机有改动。同步关闭时什么也不做。可以在任何线程调用，但跨进程时会做一次 binder 调用，不要放在按键路径上。
     *
     * @return 标记是否已经送达；provider 不可达时返回 false，调用方不需要重试，下次打开同步时会整份比对
     */
    public static boolean markDirty(Context context, String section) {
        if (!SyncSwitch.validSection(section)) throw new IllegalArgumentException("unknown sync section");
        Context application = context.getApplicationContext();
        if (AccountSessionRoutingPolicy.ownsSession(
                Application.getProcessName(), application.getPackageName())) {
            SyncSwitch.markDirty(application, section);
            return true;
        }
        try {
            Bundle reply = application.getContentResolver().call(providerUri(application),
                AccountSessionRoutingPolicy.METHOD_SYNC_DIRTY, section, null);
            return reply != null && reply.getBoolean(AccountSessionRoutingPolicy.KEY_SYNC_MARKED, false);
        } catch (RuntimeException unavailable) {
            return false;
        }
    }

    /** 当前同步开关与登录方式；读不到时按关闭处理。 */
    public static State state(Context context) {
        Context application = context.getApplicationContext();
        if (AccountSessionRoutingPolicy.ownsSession(
                Application.getProcessName(), application.getPackageName())) {
            return stateOf(SyncSwitch.enabled(application), SyncSwitch.loginKind(application));
        }
        try {
            Bundle reply = application.getContentResolver().call(providerUri(application),
                AccountSessionRoutingPolicy.METHOD_SYNC_STATE, null, null);
            if (reply == null) return State.OFF;
            return stateOf(reply.getBoolean(AccountSessionRoutingPolicy.KEY_SYNC_ENABLED, false),
                reply.getString(AccountSessionRoutingPolicy.KEY_LOGIN_KIND));
        } catch (RuntimeException unavailable) {
            return State.OFF;
        }
    }

    /** 把读到的两个值收成一个状态：开关只有在登录方式是真实账号时才算打开。 */
    static State stateOf(boolean enabled, String loginKind) {
        if (!SyncSwitch.validLoginKind(loginKind)) return State.OFF;
        return new State(enabled, loginKind);
    }

    private static Uri providerUri(Context application) {
        return Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName()));
    }
}
