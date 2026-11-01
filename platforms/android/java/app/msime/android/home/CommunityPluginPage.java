package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.policy.PluginCatalogPolicy;

/**
 * 社区里的一个插件：摘要、安装、评分、下架、举报和屏蔽作者，与共享设置页 `community-plugin-detail.tsx` 同样的内容和文字。
 *
 * <p>全部经 `msime_client_community` 的 plugin_* 操作（{@link PluginStore}）：详情 `plugin_detail`，安装 `plugin_install`（要登录；下载、校验后整包替换同类型同 id 的已装包，替换前先问），评分 `plugin_rate`，下架自己的作品 `plugin_delete`（已装的包保留），举报 `report`（登录账号优先，否则本机匿名账号），屏蔽作者 `block`（只用登录账号，见 {@link CommunityBlockDialog}）。装上的是按键旋律时说明手机上不播。
 *
 * <p>参数 `id` 是发布的 UUID；页面只按它读详情和显示，外部深链带来的参数不会触发安装、评分、下架、举报或屏蔽，每一个都要用户在页面上点。
 */
public final class CommunityPluginPage extends ReloadingDetailPage {
    private static final String ARG_ID = "id";

    static void open(Context context, String id) {
        Bundle args = new Bundle();
        args.putString(ARG_ID, id);
        SettingsNavigator.open(context, PageId.COMMUNITY_PLUGIN, args);
    }

    @Nullable private PluginStore.CommunityPlugin plugin;
    /** 本机已经装着同类型、同 id 的包：安装会整包替换它，先问。 */
    private boolean installedLocally;
    private boolean installed;
    @Nullable private PluginCatalogPolicy.Pack installedPack;
    private boolean busy;
    private boolean loading = true;
    private String error = "";
    private boolean signIn;
    private String notice = "";

    private String id() {
        Bundle args = getArguments();
        return args == null ? "" : args.getString(ARG_ID, "");
    }

    /** 读一次详情，同时看本机装没装同 id 的包；安装、评分之后也走这里。 */
    @Override protected void reload() {
        String id = id();
        loading = plugin == null;
        render();
        HostTask.runNetwork(this, context -> {
            PluginStore.Result<PluginStore.CommunityPlugin> detail = PluginStore.communityDetail(context, id);
            boolean local = false;
            if (detail.succeeded()) {
                PluginStore.Catalog catalog = PluginStore.catalog(context);
                local = catalog != null && catalog.find(detail.value().kind(), detail.value().pluginId()) != null;
            }
            return new Object[] {detail, local};
        }, result -> {
            loading = false;
            if (result == null) {
                error = PluginCatalogPolicy.communityMessage("", "", false);
            } else {
                @SuppressWarnings("unchecked")
                PluginStore.Result<PluginStore.CommunityPlugin> detail = (PluginStore.Result<PluginStore.CommunityPlugin>) result[0];
                if (detail.succeeded()) {
                    plugin = detail.value();
                    installedLocally = Boolean.TRUE.equals(result[1]);
                    error = "";
                    signIn = false;
                } else {
                    error = PluginCatalogPolicy.communityMessage(detail.code(), detail.detail(), false);
                    signIn = PluginCatalogPolicy.needsSignIn(detail.code());
                }
            }
            render();
        });
    }

    /** 回到这一页时不重读：安装状态和刚写的提示留着。 */
    @Override protected boolean shouldReloadWhenVisible() {
        return false;
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        Context context = requireContext();
        PluginStore.CommunityPlugin current = plugin;
        if (current == null) {
            GroupCard status = GroupCard.add(target, null);
            status.note(loading ? "正在读取插件详情…" : error);
            if (!loading && signIn) status.button("登录水杉账号", null, "登录", this::signIn);
            return;
        }
        setTitle(current.name());

        GroupCard summary = GroupCard.add(target, null);
        StringBuilder parts = new StringBuilder(PluginCatalogPolicy.kindLabel(current.kind()));
        if (!current.author().isEmpty()) parts.append(" · ").append(current.author());
        if (!current.version().isEmpty()) parts.append(" · v").append(current.version());
        summary.value(parts.toString(), null, "removed".equals(current.moderation()) ? "已下架" : current.owned() ? "我的作品" : null);
        if (!current.description().isEmpty()) summary.note(current.description());
        StringBuilder metrics = new StringBuilder(current.pluginId());
        if (!current.license().isEmpty()) metrics.append(" · 授权 ").append(current.license());
        metrics.append(" · ").append(PluginCatalogPolicy.megabytes(current.size()));
        summary.note(metrics.toString());
        summary.note("使用 " + current.downloads() + " 次" + (current.ratingCount() > 0
            ? " · ★" + String.format(java.util.Locale.ROOT, "%.1f", current.ratingAverage()) + "（" + current.ratingCount() + " 人评分）"
            : " · 暂无评分"));

        if (!error.isEmpty() || !notice.isEmpty()) {
            GroupCard message = GroupCard.add(target, null);
            if (!notice.isEmpty()) message.note(notice);
            if (!error.isEmpty()) message.note(error);
            if (signIn) message.button("登录水杉账号", null, "登录", this::signIn);
        }

        GroupCard install = GroupCard.add(target, null);
        if (installed) {
            install.note(installedPack != null && !PluginCatalogPolicy.usedOnPhone(installedPack)
                ? "已安装到插件目录。这是按键旋律，这台设备不播放，可以在电脑上使用。"
                : "已安装到插件目录，可在「我的插件」中选用。");
        } else {
            // 会替换本机同 id 的插件是后果，留在副标题；安装做什么收进「?」。
            GroupCard.Row row = install.button("安装", installedLocally
                ? "已安装同 id 的" + PluginCatalogPolicy.kindLabel(current.kind()) + "“" + current.pluginId() + "”，安装会整体替换它。"
                : null, installedLocally ? "替换安装" : "安装", this::install)
                .help("下载后装进本机，在「我的插件」中选用；要登录水杉账号。");
            if (row.button() != null) row.button().setEnabled(!busy);
        }

        GroupCard moderation = GroupCard.add(target, null);
        if (current.owned()) {
            GroupCard.Row row = moderation.button("下架这个插件", "下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。", "下架",
                this::confirmUnpublish);
            if (row.button() != null) row.button().setEnabled(!busy);
        } else {
            GroupCard.Row rate = moderation.button("我的评分", null,
                current.myRating() > 0 ? stars(current.myRating()) : "评分", this::pickRating).help("安装后可评，可重新选择");
            if (rate.button() != null) rate.button().setEnabled(!busy);
            moderation.button("举报", null, "举报", () -> CommunityReportDialog.show(this, current.name(),
                (worker, reason, detail) -> {
                    PluginStore.Result<Boolean> result = PluginStore.communityReport(worker, current.id(), reason, detail);
                    return result.succeeded() ? "" : PluginCatalogPolicy.communityMessage(result.code(), result.detail(), false);
                })).help("内容违规或侵权时告诉管理员");
            moderation.button("屏蔽作者", null, "屏蔽", () -> CommunityBlockDialog.show(this, current.author(),
                "plugins", current.id(), this::blocked));
        }
    }

    /** 屏蔽了这个插件的作者：关掉详情，回到的插件页重读社区列表，服务端已不再列出这位作者的插件；这位作者的皮肤也不再列出，所以先刷新键盘皮肤面板用的目录缓存（页面关掉后联网和写缓存照样做完）。 */
    private void blocked() {
        CommunitySkinCatalogue.refresh(this, true, ignored -> { });
        PluginsPage.communityChanged();
        requireActivity().getOnBackPressedDispatcher().onBackPressed();
    }

    private static String stars(int count) {
        // 不用 String.repeat：它是 API 33 的方法，minSdk 是 28。
        StringBuilder text = new StringBuilder(5);
        for (int index = 0; index < 5; index++) text.append(index < count ? '★' : '☆');
        return text.toString();
    }

    private void install() {
        PluginStore.CommunityPlugin current = plugin;
        if (current == null || busy) return;
        if (installedLocally) {
            MessageDialog.confirm(requireContext(), "替换已安装的插件",
                "已安装同 id 的" + PluginCatalogPolicy.kindLabel(current.kind()) + "“" + current.pluginId() + "”，安装会整体替换它。",
                "替换", () -> runInstall(current));
        } else {
            runInstall(current);
        }
    }

    private void runInstall(PluginStore.CommunityPlugin current) {
        busy = true;
        notice = "正在安装…";
        error = "";
        render();
        HostTask.runNetwork(this, context -> PluginStore.communityInstall(context, current), result -> {
            busy = false;
            notice = "";
            if (result == null || !result.succeeded()) {
                String code = result == null ? "" : result.code();
                error = PluginCatalogPolicy.communityMessage(code, result == null ? "" : result.detail(), false);
                signIn = PluginCatalogPolicy.needsSignIn(code);
            } else {
                installed = true;
                installedLocally = true;
                installedPack = result.value().pack();
                PluginsPage.communityChanged();
            }
            render();
        });
    }

    private void pickRating() {
        PluginStore.CommunityPlugin current = plugin;
        if (current == null) return;
        OptionSheet sheet = new OptionSheet(requireContext(), "我的评分", "安装后可评，可重新选择");
        for (int value = 5; value >= 1; value--) {
            int stars = value;
            sheet.option(stars(stars) + "（" + stars + " 星）", stars == current.myRating(), () -> rate(current, stars));
        }
        sheet.show();
    }

    private void rate(PluginStore.CommunityPlugin current, int stars) {
        busy = true;
        render();
        HostTask.runNetwork(this, context -> PluginStore.communityRate(context, current.id(), stars), result -> {
            busy = false;
            if (result == null || !result.succeeded()) {
                String code = result == null ? "" : result.code();
                error = PluginCatalogPolicy.communityMessage(code, result == null ? "" : result.detail(), false);
                signIn = PluginCatalogPolicy.needsSignIn(code);
                render();
                return;
            }
            notice = "已评分。";
            error = "";
            reload();
        });
    }

    private void confirmUnpublish() {
        PluginStore.CommunityPlugin current = plugin;
        if (current == null) return;
        MessageDialog.confirm(requireContext(), "下架这个插件",
            "下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。确定下架“" + current.name() + "”吗？", "确认下架插件", () -> {
                busy = true;
                render();
                HostTask.runNetwork(this, context -> PluginStore.communityDelete(context, current.id()), result -> {
                    busy = false;
                    if (result == null || !result.succeeded()) {
                        String code = result == null ? "" : result.code();
                        error = PluginCatalogPolicy.communityMessage(code, result == null ? "" : result.detail(), false);
                        signIn = PluginCatalogPolicy.needsSignIn(code);
                        render();
                        return;
                    }
                    PluginsPage.communityChanged();
                    MsToast.show(requireContext(), "已下架这个插件；其他用户将无法再下载。已安装的插件会保留。");
                    requireActivity().getOnBackPressedDispatcher().onBackPressed();
                });
            });
    }

    private void signIn() {
        SignIn.start(requireActivity(), failure -> {
            if (getView() == null) return;
            if (failure != null && !failure.isEmpty() && !LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(requireContext(), failure);
            }
            error = "";
            signIn = false;
            reload();
        });
    }
}
