package app.msime.android.home;

import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import java.util.List;

/**
 * 我的设备：这个账号当前有效的登录会话，每台一行（设备名、平台与版本、最近活跃），当前这台标出「本机」；点一行确认后移除。
 *
 * <p>设备名、平台和版本由服务端从登录时的 User-Agent 解析，解析不出时显示「未知设备」。移除当前这台就是退出登录：服务端撤销会话后再清掉本机会话与同步状态（{@link SignIn#signOut}），然后回到「我的」。
 */
public final class DevicesPage extends DetailPage {
    @Nullable private LinearLayout column;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        column.removeAllViews();
        GroupCard.add(column, null).note("正在读取…");
    }

    @Override protected void onBecameVisible() { reload(); }

    private void reload() {
        HostTask.run(this, context -> {
            try {
                return (Object) new DeviceDataApi(context).sessions();
            } catch (CloudApi.Failure failure) {
                return failure;
            }
        }, result -> {
            if (column == null) return;
            column.removeAllViews();
            if (result instanceof CloudApi.Failure failure) {
                GroupCard.add(column, null).note(failure.signedOut() ? "登录后可以在这里查看和移除登录过的设备。"
                    : failure.network() ? "连不上服务器，请检查网络后再试。" : "设备列表暂时读不到，请稍后再试。");
                return;
            }
            if (!(result instanceof List<?> sessions)) {
                GroupCard.add(column, null).note("设备列表暂时读不到，请稍后再试。");
                return;
            }
            render(column, sessions);
        });
    }

    private void render(LinearLayout column, List<?> sessions) {
        GroupCard group = GroupCard.add(column, "设备").withDividers(16);
        long now = System.currentTimeMillis();
        for (Object item : sessions) {
            if (!(item instanceof DeviceDataApi.Session session)) continue;
            String name = session.name().isEmpty() ? "未知设备" : session.name();
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
        HostTask.run(this, context -> {
            try {
                new DeviceDataApi(context).revokeSession(session.id());
                if (session.current()) SignIn.signOut(context);
                return "";
            } catch (CloudApi.Failure failure) {
                return failure.network() ? "连不上服务器，请检查网络后再试" : "没有移除，请稍后再试";
            }
        }, failure -> {
            if (failure == null) {
                MsToast.show(requireContext(), "没有移除，请稍后再试");
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
