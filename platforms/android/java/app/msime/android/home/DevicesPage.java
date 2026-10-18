package app.msime.android.home;

import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountStore;
import app.msime.android.DeviceNamePolicy;
import app.msime.android.DeviceDataApi;
import java.util.List;

/**
 * 我的设备：这个账号当前有效的登录会话，每台一行（设备名、平台与版本、最近活跃），当前这台标出「本机」；点一行确认后移除。
 *
 * <p>设备名、平台和版本由服务端从登录时的 User-Agent 解析，解析不出时显示「未知设备」。列表和移除经 Rust 的 sessions_list、session_revoke（{@link DeviceDataApi}）。移除当前这台就是退出登录：Rust 撤销会话并清掉本机会话，这里再清本机的同步状态（{@link SignIn#signOut}），然后回到「我的」。
 */
public final class DevicesPage extends ReloadingDetailPage {
    private record Loaded(String sessionId, List<DeviceDataApi.Session> sessions) {}
    @Nullable private String loadedSessionId;
    private long generation;

    @Override protected void onContentDestroyed() {
        generation++;
        loadedSessionId = null;
    }

    @Override protected void onContentReady() {
        showLoading();
    }

    @Override protected boolean shouldReloadOnContentReady() { return false; }

    private void showLoading() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        GroupCard.add(target, null).note("正在读取…");
    }

    @Override protected void reload() {
        long request = ++generation;
        loadedSessionId = null;
        showLoading();
        HostTask.runNetwork(this, context -> {
            try {
                String sessionId = DeviceDataApi.currentUserId(context);
                if (sessionId.isEmpty()) return new AccountStore.Failure("account_unauthorized", 0);
                return (Object) new Loaded(sessionId, new DeviceDataApi(context, sessionId).sessions());
            } catch (AccountStore.Failure failure) {
                return failure;
            }
        }, result -> {
            LinearLayout current = contentColumn();
            if (current == null || request != generation) return;
            current.removeAllViews();
            if (result instanceof AccountStore.Failure failure) {
                GroupCard.add(current, null).note(AccountFailureText.devices(failure.code, false));
                return;
            }
            if (!(result instanceof Loaded loaded)) {
                GroupCard.add(current, null).note("设备列表暂时读不到，请稍后再试。");
                return;
            }
            loadedSessionId = loaded.sessionId();
            render(current, loaded.sessions());
        });
    }

    private void render(LinearLayout column, List<?> sessions) {
        GroupCard group = GroupCard.add(column, "设备").withDividers(16);
        long now = System.currentTimeMillis();
        for (Object item : sessions) {
            if (!(item instanceof DeviceDataApi.Session session)) continue;
            String name = DeviceNamePolicy.display(session.name());
            StringBuilder detail = new StringBuilder(platformName(session.platform()));
            if (!session.appVersion().isEmpty()) detail.append(' ').append(session.appVersion());
            String active = DeviceDataApi.relativeTime(now, session.lastActive());
            if (!active.isEmpty()) detail.append(" · ").append(session.current() ? "当前在用" : "最近活跃 " + active);
            group.nav(name, detail, session.current() ? "本机" : null, () -> confirmRemove(session, name));
        }
        group.footer(sessions.isEmpty() ? "没有其他登录着的设备。"
            : "共 " + sessions.size() + " 台。移除后那台设备需要重新登录才能同步。");
    }

    private void confirmRemove(DeviceDataApi.Session session, String name) {
        new OptionSheet(requireContext(), "移除「" + name + "」",
            session.current() ? "这是本机，移除就是退出登录" : "那台设备会退出登录")
            .destructive(session.current() ? "退出登录并移除" : "移除", () -> remove(session))
            .show();
    }

    private void remove(DeviceDataApi.Session session) {
        String sessionId = loadedSessionId;
        if (sessionId == null) return;
        long request = ++generation;
        loadedSessionId = null;
        HostTask.runNetwork(this, context -> {
            try {
                boolean signedOut = new DeviceDataApi(context, sessionId).revokeSession(session.id());
                // 撤销的是本机这一次时 Rust 已经清掉本地会话，这里清本机的同步状态。
                if (signedOut) SignIn.signOut(context);
                return "";
            } catch (AccountStore.Failure failure) {
                // 撤销本机这一次时 Rust 先清本地会话再请求服务端：服务端那一步失败（断网）时本机已经退出，照样清同步状态。
                if (session.current() && signedOutLocally(context)) {
                    SignIn.signOut(context);
                    return SIGNED_OUT + AccountFailureText.signedOutLocally(failure.code);
                }
                return AccountFailureText.devices(failure.code, true);
            }
        }, failure -> {
            if (request != generation) return;
            if (failure == null) {
                MsToast.show(requireContext(), "没有移除，请稍后再试");
            } else if (failure.startsWith(SIGNED_OUT)) {
                MsToast.show(requireContext(), failure.substring(SIGNED_OUT.length()));
                requireActivity().getOnBackPressedDispatcher().onBackPressed();
            } else if (!failure.isEmpty()) {
                MsToast.show(requireContext(), failure);
            } else if (session.current()) {
                MsToast.show(requireContext(), "已退出登录");
                requireActivity().getOnBackPressedDispatcher().onBackPressed();
            } else {
                MsToast.show(requireContext(), "已移除");
                reload();
            }
        });
    }

    /** 内部记号：本机已经退出，后面那句话照样显示，然后离开本页；不会显示给用户。 */
    private static final String SIGNED_OUT = "\u0001";

    /** Rust 那边已经没有登录会话了。读不了时按还登录着处理，不去清同步状态。 */
    private static boolean signedOutLocally(android.content.Context context) {
        try {
            return DeviceDataApi.currentUserId(context).isEmpty();
        } catch (AccountStore.Failure unreadable) {
            return false;
        }
    }

    /** 服务端给的平台标识（android / ios / windows / macos / linux / harmony …）的显示名。 */
    static String platformName(String platform) {
        switch (platform == null ? "" : platform) {
            case "android": return "Android";
            case "ios": return "iOS";
            case "ipados": return "iPadOS";
            case "windows": return "Windows";
            case "macos": return "macOS";
            case "linux": return "Linux";
            case "harmony": return "HarmonyOS";
            case "harmony-pc": return "HarmonyOS 电脑";
            case "web": return "网页";
            default: return platform == null || platform.isEmpty() ? "未知平台" : platform;
        }
    }
}
