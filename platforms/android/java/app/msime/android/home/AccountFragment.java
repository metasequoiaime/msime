package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.content.res.ColorStateList;
import android.graphics.Bitmap;
import android.os.Bundle;
import android.view.Gravity;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.Switch;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.AppIconStyle;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import app.msime.android.R;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 「我的」tab：资料卡、我的内容、同步、通用，以及其他平台下载、帮助与反馈、关于、新手引导和开屏动画。
 *
 * <p>资料卡在登录真实账号（Google / Apple / 邮箱）后显示头像、昵称和「已同步 · 最近 N 分钟前」，点了进个人资料；未登录时是「?」头像和「未登录」，点了打开登录面板。云同步开关只写本机的 {@link SyncSwitch}（P9）：打开时标记全部分类待同步，首次开启的「合并 / 使用云端」确认由宿主的同步控制器在回到前台时弹出；关闭时只关开关、保留游标。没有真实账号时开关置灰，副标题是「登录后可用」。
 *
 * <p>页面先读本机的状态（偏好、同步开关、是否登录）立即画出来，再读要联网的部分（资料、设备数、云剪贴板条数），后者读不到时对应的行只是不显示数字。
 */
public final class AccountFragment extends HomeTabFragment {
    /** 本机状态：偏好、是否登录、同步开关、是否绑定了真实账号、上次同步时间、同步控制器留下的状态提示（没有时为空串）。 */
    private record Local(@Nullable JSONObject preferences, boolean signedIn, boolean syncEnabled, boolean realAccount,
            long lastSyncedAt, String syncStatus) {}

    /** 联网读到的：资料（读不到为 null）、头像、设备数与云剪贴板条数（读不到为 -1）、是否绑定了真实账号。 */
    private record Remote(@Nullable DeviceDataApi.Profile profile, @Nullable Bitmap avatar, int devices,
            int clipboard, boolean realAccount) {}

    @Nullable private Local local;
    @Nullable private Remote remote;

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_account, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        View card = view.findViewById(R.id.account_card);
        card.setBackground(Ui.rippleOn(requireContext(), Ui.card(requireContext()), Ui.dp(requireContext(), 20)));
        card.setOnClickListener(ignored -> openProfile());
        render();
    }

    // 键盘进程、同步和别的页面都可能在这一页藏着的时候改了偏好或登录状态。
    @Override protected void onBecameVisible() { reload(); }

    private void reload() {
        HostTask.run(this, context -> new Local(snapshotPreferences(context), new BackendAccount(context).signedIn(),
            SyncSwitch.enabled(context), SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)),
            SyncSwitch.lastSyncedAt(context), CloudSync.statusLine(context)), state -> {
                if (state == null) return;
                local = state;
                if (!state.signedIn()) remote = null;
                render();
                if (state.signedIn()) reloadRemote();
            });
    }

    @Nullable private static JSONObject snapshotPreferences(Context context) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        return snapshot == null ? null : snapshot.optJSONObject("preferences");
    }

    private void reloadRemote() {
        HostTask.runNetwork(this, context -> {
            DeviceDataApi api = new DeviceDataApi(context);
            DeviceDataApi.Profile profile;
            try {
                profile = api.profile();
                // 登录时没读到用户 id 的话，这里补上同步要用的账号绑定。
                ProfilePage.bindIfNeeded(context, profile);
            } catch (CloudApi.Failure failure) {
                profile = null;
            }
            int devices;
            try {
                devices = profile == null ? -1 : api.sessions().size();
            } catch (CloudApi.Failure failure) {
                devices = -1;
            }
            int clipboard;
            try {
                BackendAccount.ClipboardPage page = new BackendAccount(context).clipboard("");
                clipboard = page.enabled() ? page.items().size() : -1;
            } catch (Exception unavailable) {
                clipboard = -1;
            }
            Bitmap avatar = profile == null ? null : ProfilePage.avatar(profile.avatarUrl());
            return new Remote(profile, avatar, devices, clipboard,
                SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)));
        }, state -> {
            if (state == null) return;
            remote = state;
            Local current = local;
            if (current != null && state.realAccount() != current.realAccount()) {
                local = new Local(current.preferences(), current.signedIn(), current.syncEnabled(),
                    state.realAccount(), current.lastSyncedAt(), current.syncStatus());
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

    private static String syncLine(Local state) {
        if (!state.realAccount()) return "已登录";
        if (!state.syncEnabled()) return "云同步已关闭";
        // 最近一次失败的原因等提示优先于「已同步」，否则同步失败了页面也看不出来。
        if (!state.syncStatus().isEmpty()) return state.syncStatus();
        String ago = DeviceDataApi.relativeTime(System.currentTimeMillis(), state.lastSyncedAt());
        return ago.isEmpty() ? "云同步已开启 · 尚未同步" : "已同步 · 最近 " + ago;
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
        row(group, R.drawable.ic_ms_palette, "我的皮肤", null, skinName(state == null ? null : state.preferences()),
            () -> SettingsNavigator.open(context, PageId.SKINS, null));
        row(group, R.drawable.ic_ms_menu_book, "我的词库", null, null,
            () -> SettingsNavigator.open(context, PageId.LEXICON, null));
        row(group, R.drawable.ic_ms_star, "常用语", null, null,
            () -> SettingsNavigator.open(context, PageId.PHRASES, null));
        row(group, R.drawable.ic_ms_content_paste, "云剪贴板", null,
            online == null || online.clipboard() < 0 ? null : online.clipboard() + " 条",
            () -> SettingsNavigator.open(context, PageId.CLOUD_CLIPBOARD, null));
        // 社区作品的管理界面只在 Tauri 合包里有（P21），保留原来的跳转。
        if (tauriAvailable()) {
            row(group, R.drawable.ic_ms_groups, "社区作品", "发布、收藏皮肤、词库和回复", null, this::openCommunityAccount);
        }
    }

    /** 当前全局主题在共享目录里的名字；读不到时不显示。 */
    @Nullable private static String skinName(@Nullable JSONObject preferences) {
        if (preferences == null) return null;
        String id = preferences.optString("global_theme", "system");
        JSONArray themes = HostStore.themeCatalog();
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry != null && id.equals(entry.optString("id", ""))) return entry.optString("title", id);
        }
        return "custom".equals(id) ? "自定义" : null;
    }

    private void openCommunityAccount() {
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "account");
        startActivity(intent);
    }

    /** 独立的原生 APK 没有 WebView；Tauri 合包有。 */
    private boolean tauriAvailable() {
        try {
            Class.forName("app.msime.android.MainActivity");
            return true;
        } catch (ClassNotFoundException error) {
            return false;
        }
    }

    // ---- 同步 ----

    private void bindSync(LinearLayout groups) {
        Context context = requireContext();
        Local state = local;
        Remote online = remote;
        boolean signedIn = state != null && state.signedIn();
        boolean real = signedIn && state.realAccount();
        GroupCard group = GroupCard.add(groups, "同步").withDividers(56);
        LinearLayout sync = row(group, R.drawable.ic_ms_sync, "云同步",
            real ? "词库、自造词和设置在设备间同步" : "登录后可用", null, null);
        MsSwitch toggle = new MsSwitch(context);
        toggle.setChecked(real && state.syncEnabled());
        toggle.setClickable(false);
        toggle.setFocusable(false);
        toggle.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams switchParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        switchParams.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
        sync.addView(toggle, switchParams);
        sync.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Switch.class.getName());
                info.setCheckable(true);
                info.setChecked(toggle.isChecked());
            }
        });
        if (real) {
            sync.setBackground(Ui.ripple(context));
            sync.setClickable(true);
            sync.setFocusable(true);
            sync.setOnClickListener(ignored -> setSync(!toggle.isChecked()));
        } else {
            Ui.setEnabledLook(sync, false);
        }

        String devices = !signedIn ? "—" : online == null || online.devices() < 0 ? null : online.devices() + " 台";
        row(group, R.drawable.ic_ms_devices, "我的设备", null, devices,
            signedIn ? () -> SettingsNavigator.open(context, PageId.DEVICES, null) : null);
    }

    private void setSync(boolean enabled) {
        HostTask.run(this, context -> {
            SyncSwitch.setEnabled(context, enabled);
            // 打开时把全部分类标成待上传；比对与首次合并确认由宿主的同步控制器负责。
            if (enabled) for (String section : SyncSwitch.SECTIONS) SyncSignals.markDirty(context, section);
            return enabled;
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
        AppIconStyle icon = AppIcons.selected(context);
        row(group, R.drawable.ic_ms_smartphone, "App 图标", null, icon.title(), this::showIcons);
    }

    // ---- 收尾组 ----

    private void bindMore(LinearLayout groups) {
        Context context = requireContext();
        GroupCard group = GroupCard.add(groups, null).withDividers(56);
        row(group, R.drawable.ic_ms_download, "其他平台下载", null, "7 个平台",
            () -> SettingsNavigator.open(context, PageId.DOWNLOAD, null));
        row(group, R.drawable.ic_ms_feedback, "帮助与反馈", null, null,
            () -> SettingsNavigator.open(context, PageId.FEEDBACK, null));
        row(group, R.drawable.ic_ms_info, "关于", null, version(),
            () -> SettingsNavigator.open(context, PageId.ABOUT, null));
        row(group, R.drawable.ic_ms_keyboard, "新手引导", "四步走完键盘的启用和设置", null,
            () -> startActivity(new Intent(context, OnboardingActivity.class)));
        // 「开屏动画」只重播开屏，不会接着进入新手引导。
        row(group, R.drawable.ic_ms_circle, "开屏动画", null, "播放", () -> {
            if (getActivity() instanceof HomeActivity home) home.replaySplash();
        });
    }

    /** 已安装的版本名；包管理器不肯说时是一道横线。 */
    private String version() {
        try {
            String name = requireContext().getPackageManager()
                .getPackageInfo(requireContext().getPackageName(), 0).versionName;
            return name == null ? "—" : name;
        } catch (android.content.pm.PackageManager.NameNotFoundException error) {
            return "—";
        }
    }

    private void showIcons() {
        Context context = requireContext();
        AppIconStyle current = AppIcons.selected(context);
        OptionSheet sheet = new OptionSheet(context, "App 图标",
            "切换时桌面图标会短暂消失再出现，这是系统在重建启动项");
        for (AppIconStyle style : AppIconStyle.all()) {
            sheet.option(style.title() + " · " + style.description(), style == current, () -> {
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
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setMinimumHeight(Ui.dp(context, subtitle == null ? 52 : Ui.ROW_MIN_HEIGHT));
        row.setPadding(Ui.dp(context, Ui.ROW_PADDING_H), Ui.dp(context, Ui.ROW_PADDING_V),
            Ui.dp(context, Ui.ROW_PADDING_H), Ui.dp(context, Ui.ROW_PADDING_V));

        ImageView glyph = new ImageView(context);
        glyph.setImageResource(icon);
        glyph.setImageTintList(ColorStateList.valueOf(Ui.subText(context)));
        glyph.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams glyphParams = new LinearLayout.LayoutParams(Ui.dp(context, 22), Ui.dp(context, 22));
        glyphParams.setMarginEnd(Ui.dp(context, 18));
        row.addView(glyph, glyphParams);

        LinearLayout texts = new LinearLayout(context);
        texts.setOrientation(LinearLayout.VERTICAL);
        TextView heading = new TextView(context);
        heading.setText(title);
        Ui.style(heading, Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
        texts.addView(heading);
        if (subtitle != null) {
            TextView detail = new TextView(context);
            detail.setText(subtitle);
            Ui.style(detail, 12, 400, Ui.subText(context));
            texts.addView(detail);
        }
        row.addView(texts, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        if (value != null && value.length() > 0) {
            TextView trailing = new TextView(context);
            trailing.setText(value);
            trailing.setSingleLine(true);
            Ui.style(trailing, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
            LinearLayout.LayoutParams valueParams = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            valueParams.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
            row.addView(trailing, valueParams);
        }
        if (action != null) {
            ImageView chevron = new ImageView(context);
            chevron.setImageResource(R.drawable.ms_w1_a2_chevron);
            chevron.setImageTintList(ColorStateList.valueOf(Ui.subText(context)));
            chevron.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
            LinearLayout.LayoutParams chevronParams = new LinearLayout.LayoutParams(
                Ui.dp(context, Ui.CHEVRON_SIZE), Ui.dp(context, Ui.CHEVRON_SIZE));
            chevronParams.setMarginStart(Ui.dp(context, 6));
            row.addView(chevron, chevronParams);
            row.setBackground(Ui.ripple(context));
            row.setClickable(true);
            row.setFocusable(true);
            row.setOnClickListener(ignored -> action.run());
        }
        group.addView(row);
        return row;
    }
}
