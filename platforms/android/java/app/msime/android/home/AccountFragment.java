package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.DrawablePolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.LinearLayout;
import android.widget.Switch;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.AppIconStyle;
import app.msime.android.AppVersionPolicy;
import app.msime.android.BackendAccount;
import app.msime.android.AccountStore;
import app.msime.android.CloudClipboardNativeDriver;
import app.msime.android.CloudClipboardRelay;
import app.msime.android.DeviceDataApi;
import app.msime.android.R;
import app.msime.android.RustCloudSync;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 「我的」tab：资料卡、我的内容、同步、通用，以及其他平台下载、帮助与反馈、关于、新手引导和开屏动画。
 *
 * <p>资料卡在登录真实账号（Google / Apple / 邮箱）后显示头像、昵称和「已同步 · 最近 N 分钟前」，点了进个人资料；未登录时是「?」头像和「未登录」，点了打开登录面板。云同步开关经 Rust 的 `set_enabled`（{@link CloudSync#setEnabled}）：打开时 Rust 把全部分类标成待上传，马上比对一次，首次开启的「合并 / 使用云端」确认由宿主的同步控制器弹出；关闭时只关开关、保留游标。没有真实账号时开关置灰，副标题是「登录后可用」。
 *
 * <p>页面先读本机的状态（偏好、同步开关、是否登录）立即画出来，再读要联网的部分（资料、设备数、云剪贴板条数），后者读不到时对应的行只是不显示数字。
 */
public final class AccountFragment extends HomeTabFragment {
    /** 本机状态：偏好、是否登录、同步开关、是否绑定了真实账号、Rust 写好的同步状态行（读不到时为空串）。 */
    private record Local(@Nullable JSONObject preferences, boolean signedIn, boolean syncEnabled, boolean realAccount,
            String syncStatus) {}

    /** 联网读到的：资料（读不到为 null）、头像、设备数与云剪贴板条数（读不到为 -1）、是否绑定了真实账号、读的是哪个账号，以及读完时登录的是否仍是它（在工作线程上核对，主线程不调 Rust）。 */
    private record Remote(@Nullable DeviceDataApi.Profile profile, @Nullable Bitmap avatar, int devices,
            int clipboard, boolean realAccount, String sessionId, boolean current) {}

    @Nullable private Local local;
    @Nullable private Remote remote;
    private long generation;

    @Override public void onDestroyView() {
        generation++;
        local = null;
        remote = null;
        super.onDestroyView();
    }

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_account, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        View card = view.findViewById(R.id.account_card);
        ViewPolicy.setBackground(card, ViewPolicy.ripple(requireContext(), ThemeColorPolicy.card(requireContext()), DimensionPolicy.pixels(requireContext(), 20)));
        ViewPolicy.bindClick(card, this::openProfile);
        render();
    }

    // 键盘进程、同步和别的页面都可能在这一页藏着的时候改了偏好或登录状态。
    @Override protected void onBecameVisible() { reload(); }

    private void reload() {
        long request = ++generation;
        HostTask.run(this, context -> {
            RustCloudSync.Status sync = CloudSync.status(context);
            return new Local(KeyboardSheets.preferences(context), new BackendAccount(context).signedIn(),
                sync.enabled(), SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)), sync.line());
        }, state -> {
                if (state == null || request != generation || getView() == null) return;
                local = state;
                remote = null;
                render();
                if (state.signedIn()) reloadRemote(request);
            });
    }

    private void reloadRemote(long request) {
        HostTask.runNetwork(this, context -> {
            String sessionId;
            try {
                sessionId = DeviceDataApi.currentUserId(context);
            } catch (AccountStore.Failure unavailable) {
                return new Remote(null, null, -1, -1, false, "", false);
            }
            if (sessionId.isEmpty()) return new Remote(null, null, -1, -1, false, "", false);
            DeviceDataApi api = new DeviceDataApi(context, sessionId);
            DeviceDataApi.Profile profile;
            try {
                profile = api.profile();
                // 登录时没读到用户 id 的话，这里补上同步要用的账号绑定。
                ProfilePage.bindIfNeeded(context, profile, sessionId);
            } catch (AccountStore.Failure failure) {
                profile = null;
            }
            int devices;
            try {
                devices = profile == null ? -1 : api.sessions().size();
            } catch (AccountStore.Failure failure) {
                devices = -1;
            }
            int clipboard;
            try {
                if (CloudClipboardRelay.selected(CloudClipboardRelay.preferencesDirectory(context))) {
                    // 云剪贴板改走了自建中继或 WebDAV 网盘：这里是账号页，不为了一个条数去连中继或网盘，更不能去问水杉账号（那会把这台设备登记到账号的云剪贴板上）。
                    clipboard = -1;
                } else {
                    BackendAccount.ClipboardPage page = new BackendAccount(context).clipboard("", sessionId,
                        CloudClipboardNativeDriver.local(context));
                    // 这台设备还在等允许时读不到条目，不写「0 条」。
                    clipboard = page.enabled() && !page.pending() ? page.items().size() : -1;
                }
            } catch (Exception unavailable) {
                clipboard = -1;
            }
            Bitmap avatar = profile == null ? null : ProfilePage.avatar(api, profile);
            boolean current;
            try {
                current = sessionId.equals(DeviceDataApi.currentUserId(context));
            } catch (AccountStore.Failure unavailable) {
                current = false;
            }
            return new Remote(profile, avatar, devices, clipboard,
                SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)), sessionId, current);
        }, state -> {
            if (state == null || request != generation || getView() == null) return;
            if (state.sessionId().isEmpty() || state.profile() == null || !state.current()
                    || !state.profile().id().equals(SyncSwitch.accountId(requireContext()))) return;
            remote = state;
            Local current = local;
            if (current != null && state.realAccount() != current.realAccount()) {
                local = new Local(current.preferences(), current.signedIn(), current.syncEnabled(),
                    state.realAccount(), current.syncStatus());
            }
            render();
        });
    }

    private void render() {
        View view = getView();
        if (view == null) return;
        bindCard(view);
        LinearLayout groups = view.findViewById(R.id.account_groups);
        groups.removeAllViews();
        bindContent(groups);
        bindSync(groups);
        bindGeneral(groups);
        bindMore(groups);
    }

    // ---- 资料卡 ----

    private void bindCard(View view) {
        Context context = requireContext();
        Local state = local;
        Remote online = remote;
        boolean signedIn = state != null && state.signedIn();
        DeviceDataApi.Profile profile = online == null ? null : online.profile();
        String name = !signedIn ? "未登录" : profile == null ? "已登录" : profile.displayName();
        ((TextView) view.findViewById(R.id.account_title)).setText(name);
        ((TextView) view.findViewById(R.id.account_subtitle)).setText(state == null ? ""
            : !signedIn ? "登录后同步词库、皮肤和设置" : syncLine(state));
        ViewGroup avatar = view.findViewById(R.id.account_avatar);
        avatar.removeAllViews();
        avatar.addView(ProfilePage.avatarView(context, 56, signedIn ? name : "",
            online == null ? null : online.avatar()));
        view.findViewById(R.id.account_card).setContentDescription(
            signedIn ? name + "，个人资料" : "未登录，点按登录");
    }

    /** 资料卡下面那一行。登录了真实账号时用 Rust 写好的同步状态行：失败原因、云端被清空等提示优先于「已同步 · 最近 …」，关闭时是「云同步已关闭」（在「云端被清空」里选了关闭同步时另有一句）；读不到状态时只按开关说。 */
    private static String syncLine(Local state) {
        if (!state.realAccount()) return "已登录";
        if (!state.syncStatus().isEmpty()) return state.syncStatus();
        return state.syncEnabled() ? "云同步已开启" : "云同步已关闭";
    }

    private void openProfile() {
        Local state = local;
        if (state != null && state.signedIn()) {
            SettingsNavigator.open(requireContext(), PageId.PROFILE, null);
            return;
        }
        SignIn.start(requireActivity(), failure -> {
            // 登录面板可能在用户离开这一页之后才结束；没有视图就不再汇报。
            if (!isAdded() || getView() == null) return;
            if (failure.isEmpty()) {
                MsToast.show(requireContext(), "已登录");
                reload();
            } else if (!LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(requireContext(), failure);
            }
        });
    }

    // ---- 我的内容 ----

    private void bindContent(LinearLayout groups) {
        Context context = requireContext();
        Local state = local;
        Remote online = remote;
        GroupCard group = GroupCard.add(groups, "我的内容").withDividers(56);
        row(group, R.drawable.ic_ms_palette, "我的皮肤", null, skinName(state == null ? null : state.preferences(),
                AppMode.dark(requireContext())),
            () -> SettingsNavigator.open(context, PageId.SKINS, null));
        row(group, R.drawable.ic_ms_menu_book, "我的词库", null, null,
            () -> SettingsNavigator.open(context, PageId.LEXICON, null));
        KeyboardSheets.help(row(group, R.drawable.ic_ms_sync, "词库同步", null, null,
            () -> SettingsNavigator.open(context, PageId.DICTIONARY_SYNC, null)), "词库同步", "「我的词条」经自己的 WebDAV 网盘同步");
        row(group, R.drawable.ic_ms_star, "常用语", null, null,
            () -> SettingsNavigator.open(context, PageId.PHRASES, null));
        KeyboardSheets.help(row(group, R.drawable.ic_ms_download, "备份与恢复", null, null,
            () -> SettingsNavigator.open(context, PageId.BACKUP, null)), "备份与恢复", "导出到本机文件，换手机时恢复");
        row(group, R.drawable.ic_ms_content_paste, "云剪贴板", null,
            online == null || online.clipboard() < 0 ? null : online.clipboard() + " 条",
            () -> SettingsNavigator.open(context, PageId.CLOUD_CLIPBOARD, null));
        // 社区作品的管理界面只在 Tauri 合包里有（P21），保留原来的跳转。
        if (ManagementUi.available()) {
            KeyboardSheets.help(row(group, R.drawable.ic_ms_groups, "社区作品", null, null, this::openCommunityAccount),
                "社区作品", "发布、收藏皮肤、词库和回复");
        }
    }

    /** 当前键盘明暗对应的皮肤名；设计库引用由皮肤页显示具体名字。 */
    @Nullable private static String skinName(@Nullable JSONObject preferences, boolean systemDark) {
        if (preferences == null) return null;
        boolean dark = app.msime.android.KeyboardSkin.resolveDark(
            preferences.optString("screen_keyboard_theme", "follow"),
            preferences.optString("theme", "system"), systemDark);
        String modeChoice = preferences.optString(app.msime.android.KeyboardSkinModeChoice.key(dark), "");
        if (modeChoice.startsWith("design:")) return "我的皮肤";
        String id = modeChoice.startsWith("theme:") ? modeChoice.substring("theme:".length())
            : InputViewValuePolicy.textOr(preferences, "global_theme", "system");
        JSONArray themes = HostStore.themeCatalog();
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry != null && id.equals(InputViewValuePolicy.textOr(entry, "id", "")))
                return InputViewValuePolicy.textOr(entry, "title", id);
        }
        return "custom".equals(id) ? "自定义" : null;
    }

    private void openCommunityAccount() {
        startActivity(ManagementUi.settingsPage(requireContext(), "account"));
    }

    // ---- 同步 ----

    private void bindSync(LinearLayout groups) {
        Context context = requireContext();
        Local state = local;
        Remote online = remote;
        boolean signedIn = state != null && state.signedIn();
        boolean real = signedIn && state.realAccount();
        GroupCard group = GroupCard.add(groups, "同步").withDividers(56);
        // 「登录后可用」是这一行点不了的原因，留在副标题；功能说明收进「?」。
        LinearLayout sync = KeyboardSheets.help(row(group, R.drawable.ic_ms_sync, "云同步",
            real ? null : "登录后可用", null, null), "云同步", "词库、自造词和设置在设备间同步");
        MsSwitch toggle = new MsSwitch(context);
        toggle.setChecked(real && state.syncEnabled());
        ViewPolicy.setInteractive(toggle, false);
        ViewPolicy.hideFromAccessibility(toggle);
            LinearLayout.LayoutParams switchParams = LayoutPolicy.rowGapParams(context, Ui.ROW_GAP);
        sync.addView(toggle, switchParams);
        sync.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Switch.class.getName());
                info.setCheckable(true);
                androidx.core.view.accessibility.AccessibilityNodeInfoCompat.wrap(info).setChecked(toggle.isChecked());
            }
        });
        if (real) {
            ViewPolicy.setBackground(sync, ViewPolicy.ripple(context));
            ViewPolicy.setInteractive(sync, true);
            ViewPolicy.bindClick(sync, () -> setSync(!toggle.isChecked()));
        } else {
            ViewPolicy.setEnabledWithAlpha(sync, false, 0.38f);
        }

        String devices = !signedIn ? "—" : online == null || online.devices() < 0 ? null : online.devices() + " 台";
        row(group, R.drawable.ic_ms_devices, "我的设备", null, devices,
            signedIn ? () -> SettingsNavigator.open(context, PageId.DEVICES, null) : null);
        // 绑定后在 QQ 群里发 /stats 看打字数据；只有真实账号能绑定。
        LinearLayout qq = KeyboardSheets.help(row(group, R.drawable.ic_ms_link, "绑定 QQ 群助手",
            real ? null : "登录后可用", null,
            real ? () -> SettingsNavigator.open(context, PageId.QQ_LINKS, null) : null), "绑定 QQ 群助手", "在群里发 /stats 查看打字数据");
        if (!real) ViewPolicy.setEnabledWithAlpha(qq, false, 0.38f);
    }

    private void setSync(boolean enabled) {
        HostTask.run(this, context -> {
            // 打开时 Rust 把全部分类标成待上传；比对与首次合并确认由宿主的同步控制器负责。
            try {
                CloudSync.setEnabled(context, enabled);
                return enabled;
            } catch (AccountStore.Failure failure) {
                return null;
            }
        }, done -> {
            if (done == null) {
                MsToast.show(requireContext(), "登录后才能打开云同步");
            } else {
                MsToast.show(requireContext(), done ? "云同步已开启" : "云同步已关闭");
                // 打开后马上比对一次，首次的「合并 / 使用云端」选择也当场弹出，不等下次回到前台。
                if (done) CloudSync.runNow(requireActivity());
            }
            reload();
        });
    }

    // ---- 通用 ----

    private void bindGeneral(LinearLayout groups) {
        Context context = requireContext();
        Local state = local;
        JSONObject preferences = state == null ? null : state.preferences();
        GroupCard group = GroupCard.add(groups, "通用").withDividers(56);
        row(group, R.drawable.ic_ms_brush, "应用主题", null, AppThemeSheet.summary(context, preferences),
            preferences == null ? null : () -> AppThemeSheet.show(this, preferences, this::reload));
        row(group, R.drawable.ic_ms_shield_lock, "隐私", null, "本地优先",
            () -> SettingsNavigator.open(context, PageId.PRIVACY, null));
        group.toggle("统计页身份验证", null,
            StatisticsAuthenticationPreference.enabled(context),
            enabled -> StatisticsAuthenticationPreference.save(context, enabled)).help("开启后使用系统认证，5 分钟内再次进入不用重新验证；未配置锁屏凭据时直接进入");
        AppIconStyle icon = AppIcons.selected(context);
        group.addView(KeyboardSheets.iconNavRow(context, context.getDrawable(AppIcons.resource(icon)),
            "App 图标", null, icon.title(), this::showIcons));
    }

    // ---- 收尾组 ----

    private void bindMore(LinearLayout groups) {
        Context context = requireContext();
        GroupCard group = GroupCard.add(groups, null).withDividers(56);
        row(group, R.drawable.ic_ms_download, "其他平台下载", null, "7 个平台",
            () -> SettingsNavigator.open(context, PageId.DOWNLOAD, null));
        row(group, R.drawable.ic_ms_feedback, "帮助与反馈", null, null,
            () -> SettingsNavigator.open(context, PageId.FEEDBACK, null));
        LinearLayout about = row(group, R.drawable.ic_ms_info, "关于", null, version(),
            () -> SettingsNavigator.open(context, PageId.ABOUT, null));
        about.setOnLongClickListener(ignored -> {
            AboutEasterEgg.show(context);
            return true;
        });
        KeyboardSheets.help(row(group, R.drawable.ic_ms_keyboard, "新手引导", null, null,
            () -> startActivity(new Intent(context, OnboardingActivity.class))), "新手引导", "四步走完键盘的启用和设置");
        // 「开屏动画」只重播开屏，不会接着进入新手引导。
        row(group, R.drawable.ic_ms_circle, "开屏动画", null, "播放", () -> {
            if (getActivity() instanceof HomeActivity home) home.replaySplash();
        });
    }

    /** 已安装的版本名；包管理器不肯说时是一道横线。 */
    private String version() {
        return AppVersionPolicy.versionName(requireContext(), "—");
    }

    private void showIcons() {
        Context context = requireContext();
        AppIconStyle current = AppIcons.selected(context);
        AppIconSheet sheet = new AppIconSheet(context);
        for (AppIconStyle style : AppIconStyle.all()) {
            sheet.option(style, style == current, () -> {
                boolean changed = AppIcons.select(context, style);
                MsToast.show(context, changed
                    ? "已切换为「" + style.title() + "」，桌面图标稍后更新"
                    : "系统拒绝了这次切换，图标保持不变");
                render();
            });
        }
        sheet.show();
    }

    // ---- 行 ----

    /**
     * 设计里「我的」的行：前面 22dp 线框图标，标题（可带副标题），行尾是值和 ›；`action` 为 null 时没有 ›，整行不可点。
     *
     * @return 行本身，开关行在它末尾再加开关
     */
    private LinearLayout row(GroupCard group, @DrawableRes int icon, CharSequence title,
            @Nullable CharSequence subtitle, @Nullable CharSequence value, @Nullable Runnable action) {
        Context context = requireContext();
        LinearLayout row = KeyboardSheets.iconNavRow(context, icon, title, subtitle, value, action);
        group.addView(row);
        return row;
    }
}
