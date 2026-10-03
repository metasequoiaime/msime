package app.msime.android.home;

import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import app.msime.android.AccountIdentity;
import app.msime.android.AppIconStyle;
import app.msime.android.BackendAccount;
import app.msime.android.R;
import com.google.android.material.imageview.ShapeableImageView;
import com.google.android.material.snackbar.Snackbar;

/**
 * The 我的 tab: who this device is to the backend, what the app looks like, and where content is.
 *
 * 这台设备的身份是自己生成的，日常使用不需要登录。What it shows first is the device's own anonymous identity -- the one the community catalogue is read with -- then Google sign-in when the backend offers it and this build carries a client ID (see {@link SignIn}), and rows for the things this host can actually do.
 */
public final class AccountFragment extends HomeTabFragment {
    private final SignInAttemptPolicy signInAttempt = new SignInAttemptPolicy();

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_account, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        render();
    }

    // The icon may have been changed elsewhere, and the keyboard may have created its identity
    // while this screen was in the background.
    @Override protected void onBecameVisible() { render(); }

    private void render() {
        View view = getView();
        if (view == null) return;
        HostTask.run(this, AccountIdentity::subject, subject -> bind(subject == null ? "" : subject));
        bindSignIn();
        bindIcons();
        bindContent();
        ((TextView) view.findViewById(R.id.account_note)).setText(
            "这个身份由本机自动生成，不需要注册或登录。它只用来读取社区目录，不携带你的输入内容，也不在设备之间同步。");
    }

    private void bind(String subject) {
        View view = getView();
        if (view == null) return;
        TextView subtitle = view.findViewById(R.id.account_subtitle);
        subtitle.setText(subject.isEmpty()
            ? "本机身份读取失败"
            : "本机身份 " + AccountIdentity.shortSubject(subject));
    }

    /**
     * 登录那一块。
     *
     * <p>Three states and they are not the same sentence: signed in, offered, and absent. {@link SignIn#state} decides which; absent draws nothing.
     */
    private void bindSignIn() {
        View view = getView();
        if (view == null) return;
        LinearLayout rows = view.findViewById(R.id.account_sign_in_rows);
        rows.removeAllViews();
        HostTask.run(this, SignIn::state, state -> {
            View current = getView();
            if (current == null || state == null || state == SignIn.State.ABSENT) return;
            LinearLayout list = current.findViewById(R.id.account_sign_in_rows);
            list.removeAllViews();
            View row = state == SignIn.State.SIGNED_IN
                ? ListRows.add(list, R.drawable.ic_tab_account,
                    getString(R.string.account_signed_in), getString(R.string.account_sign_out),
                    this::signOut)
                : ListRows.add(list, R.drawable.ic_tab_account,
                    getString(R.string.account_sign_in_google),
                    getString(R.string.account_sign_in_hint), this::signIn);
            row.setEnabled(!signInAttempt.active());
            // Inside the account card the row keeps the card's 16dp inset, so its glyph lines up with the avatar above it.
            int inset = ListRows.dp(requireContext(), 16);
            row.setPaddingRelative(inset, row.getPaddingTop(), inset, row.getPaddingBottom());
        });
    }

    private void signIn() {
        if (!signInAttempt.begin()) return;
        SignIn.start(requireActivity(), failure -> {
            signInAttempt.finish();
            // Credential Manager may finish after the user has left this tab; a detached fragment has no view to report into.
            if (!isAdded() || getView() == null) return;
            if (failure.isEmpty()) render();
            else note(failure);
        });
    }

    private void signOut() {
        HostTask.run(this, context -> {
            new BackendAccount(context).signOut();
            return "";
        }, ignored -> render());
    }

    private void note(String message) {
        View view = getView();
        if (view != null) Snackbar.make(view, message, Snackbar.LENGTH_LONG).show();
    }

    /**
     * The design's 工具, 个性化 and 我的内容 groups, holding what this host can open today.
     *
     * <p>The design's 同步 group (a cloud-sync switch and a device list) is left out: this host has no sync backend behind either, and a switch that changes nothing is worse than its absence. 我的内容 keeps only 社区作品 for the same reason -- skins, community dictionaries and phrases a user has collected are not tracked anywhere this host can read.
     */
    private void bindIcons() {
        View view = getView();
        if (view == null) return;
        LinearLayout rows = view.findViewById(R.id.account_personal_rows);
        rows.removeAllViews();
        ListRows.heading(rows, "工具");
        ListRows.add(rows, R.drawable.ic_feature_dictionary, "云剪贴板",
            "在设备之间同步你明确添加的内容", () -> startActivity(new android.content.Intent(
                requireContext(), CloudClipboardActivity.class)));
        ListRows.add(rows, R.drawable.ic_feature_dictionary, "云词库",
            "管理云端词条、个人候选和词库快照", this::openCloudDictionary);
        ListRows.add(rows, R.drawable.ic_about_desktop, "其他平台下载",
            "macOS、Windows、Linux 的安装包与指南", () -> startActivity(
                new android.content.Intent(requireContext(), DesktopDownloadActivity.class)));

        ListRows.heading(rows, "个性化");
        AppIconStyle current = AppIcons.selected(requireContext());
        ListRows.add(rows, R.drawable.ic_feature_skin, "App 图标",
            current.title() + " · " + current.description(), this::showIcons);

        ListRows.heading(rows, "我的内容");
        ListRows.add(rows, R.drawable.ic_feature_ai, "社区作品",
            "发布、收藏皮肤、词库和回复", this::openCommunityAccount);
    }

    /** Open the shared Tauri mobile panel; dictionary UI stays in the common settings surface. */
    private void openCloudDictionary() {
        if (!tauriAvailable()) {
            note("云词库需要管理界面合包，请使用 Tauri 合包打开。您仍可在本机使用词库设置。 ");
            return;
        }
        android.content.Intent intent = new android.content.Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_mobile_panel", "cloud-dictionary");
        startActivity(intent);
    }

    private void openCommunityAccount() {
        if (!tauriAvailable()) {
            note("社区管理需要管理界面合包，请使用 Tauri 合包打开。 ");
            return;
        }
        android.content.Intent intent = new android.content.Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "account");
        startActivity(intent);
    }

    /**
     * The design's closing group, which has no title: the guide, the splash, help and about.
     *
     * <p>「开屏动画」 replays the splash on the home screen. A replay is only the animation: it never leads on into onboarding, whatever the first-run state is -- the prototype did, and a user who asked to watch a logo draw itself did not ask to be walked through setup again. The design's 隐私 row is not repeated here; the privacy statement sits in 关于, which this group already opens.
     */
    private void bindContent() {
        View view = getView();
        if (view == null) return;
        LinearLayout rows = view.findViewById(R.id.account_storage_rows);
        rows.removeAllViews();
        ListRows.gap(rows);
        ListRows.add(rows, R.drawable.ic_feature_keys, "新手引导",
            "四步走完键盘的启用和设置", () -> startActivity(
                new android.content.Intent(requireContext(), OnboardingActivity.class)));
        ListRows.add(rows, R.drawable.ic_feature_skin, "开屏动画", "播放", () -> {
            if (getActivity() instanceof HomeActivity home) home.replaySplash();
        });
        ListRows.add(rows, R.drawable.ic_about_help, "帮助与反馈",
            "启用键盘、常见问题，或告诉我们哪里不好用", () -> startActivity(
                new android.content.Intent(requireContext(), HelpActivity.class)));
        ListRows.add(rows, R.drawable.ic_feature_system, "关于", version(), this::openAbout);
    }

    /** The installed version name, or a dash when the package manager will not say. */
    private String version() {
        try {
            String name = requireContext().getPackageManager()
                .getPackageInfo(requireContext().getPackageName(), 0).versionName;
            return name == null ? "—" : name;
        } catch (android.content.pm.PackageManager.NameNotFoundException error) {
            return "—";
        }
    }

    /** Keep Android's public about/help/feedback surface in the shared Tauri UI. */
    private void openAbout() {
        if (!tauriAvailable()) {
            startActivity(new android.content.Intent(requireContext(), AboutActivity.class));
            return;
        }
        android.content.Intent intent = new android.content.Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "about");
        startActivity(intent);
    }

    /** The standalone native APK deliberately has no WebView; the Tauri bundle does. */
    private boolean tauriAvailable() {
        try {
            Class.forName("app.msime.android.MainActivity");
            return true;
        } catch (ClassNotFoundException error) {
            return false;
        }
    }

    private void showIcons() {
        SettingsSheet sheet = new SettingsSheet(requireContext(), "App 图标",
            "换掉主屏幕上的水杉。切换时桌面图标会短暂消失再出现，这是系统在重建启动项。");
        AppIconStyle current = AppIcons.selected(requireContext());
        for (AppIconStyle style : AppIconStyle.all()) {
            LinearLayout row = (LinearLayout) LayoutInflater.from(requireContext())
                .inflate(R.layout.item_setting_row, sheet.content(), false);
            ShapeableImageView badge = row.findViewById(R.id.row_badge);
            // 这一行画的是图标本身，不是字形：不着色，也不要那块底。
            badge.setImageResource(icon(style));
            badge.setImageTintList(null);
            badge.getLayoutParams().width = ListRows.dp(requireContext(), 40);
            badge.getLayoutParams().height = ListRows.dp(requireContext(), 40);
            ((TextView) row.findViewById(R.id.row_title)).setText(style.title());
            ((TextView) row.findViewById(R.id.row_value)).setText(style.description());
            TextView chevron = row.findViewById(R.id.row_chevron);
            chevron.setText(style == current ? "✓" : "");
            chevron.setVisibility(View.VISIBLE);
            chevron.setTextColor(ContextCompat.getColor(requireContext(), R.color.forest));
            row.setOnClickListener(ignored -> {
                boolean changed = AppIcons.select(requireContext(), style);
                sheet.dismiss();
                bindIcons();
                View view = getView();
                if (view != null) {
                    Snackbar.make(view, changed
                        ? "已切换为「" + style.title() + "」，桌面图标稍后更新。"
                        : "系统拒绝了这次切换，图标保持不变。", Snackbar.LENGTH_LONG).show();
                }
            });
            sheet.add(row);
        }
        sheet.show();
    }

    @DrawableRes private static int icon(AppIconStyle style) {
        return switch (style) {
            case FOREST -> R.drawable.app_icon_forest;
            case SKY -> R.drawable.app_icon_sky;
            case DUSK -> R.drawable.app_icon_dusk;
            case VERMILION -> R.drawable.app_icon_vermilion;
            case CLASSIC -> R.drawable.app_icon_classic;
        };
    }
}
