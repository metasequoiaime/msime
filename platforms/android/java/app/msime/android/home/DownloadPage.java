package app.msime.android.home;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.ColorStateList;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.DownloadLinkApi;
import app.msime.android.R;
import java.util.HashSet;
import java.util.Set;

/**
 * 其他平台下载：顶上一张「在电脑上打开」卡（下载页地址与「复制链接」），下面按「电脑」「手机和平板」两组列出各个平台。
 *
 * <p>电脑上的平台带「发送链接」：服务端把这个平台的下载页链接发到账号已经验证过的邮箱（{@link DownloadLinkApi}），收件地址由服务端决定；没有登录或账号没有已验证邮箱时提示改用「复制链接」。手机和平板的平台点「获取」在浏览器里打开对应的下载页，本机所在的 Android 一行写「当前设备」。
 *
 * <p>副标题只写各平台确定的事实（分发方式、所用的输入法框架），不写会过期的版本号。
 */
public final class DownloadPage extends DetailPage {
    private static final String DOWNLOAD = "https://msime.app/download/";
    private static final String DOWNLOAD_LABEL = "msime.app/download";

    /** 本次打开页面后已经发送过的平台；按钮显示「已发送」。 */
    private final Set<String> sentPlatforms = new HashSet<>();
    private final Set<String> sending = new HashSet<>();

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();
        column.addView(hero(context));

        GroupCard desktop = GroupCard.add(column, "电脑").withDividers(58);
        sendRow(desktop, R.drawable.ic_ms_laptop, "HarmonyOS 2in1", "电脑与平板二合一 · 从源码构建", "harmony-pc");
        sendRow(desktop, R.drawable.ic_ms_desktop_windows, "Windows", "TSF 输入法 · GitHub 发布页下载", "windows");
        sendRow(desktop, R.drawable.ic_ms_laptop, "macOS", "InputMethodKit · 自带自动更新", "macos");
        sendRow(desktop, R.drawable.ic_ms_laptop, "Linux", "IBus 与 Fcitx5 · DEB、RPM 与 TGZ", "linux");

        GroupCard mobile = GroupCard.add(column, "手机和平板").withDividers(58);
        getRow(mobile, R.drawable.ic_ms_smartphone, "iOS", "TestFlight 测试版", "ios");
        getRow(mobile, R.drawable.ic_ms_tablet, "iPadOS", "与 iPhone 共用同一个 TestFlight", "ios");
        GroupCard.Row android = row(mobile, R.drawable.ic_ms_smartphone, "Android", "各版本的 APK 在 GitHub 发布页");
        TextView current = new TextView(context);
        current.setText("当前设备");
        Ui.style(current, Ui.TEXT_BUTTON_SMALL, 500, Ui.text(context));
        LinearLayout.LayoutParams currentParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        currentParams.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
        ((LinearLayout) android.view()).addView(current, currentParams);
        getRow(mobile, R.drawable.ic_ms_smartphone, "HarmonyOS", "从源码构建", "harmony");
    }

    /** 「在电脑上打开」卡：accentSoft 底的 r20 卡片，左边强调色圆角方块里一枚链接图标。 */
    private View hero(Context context) {
        LinearLayout card = new LinearLayout(context);
        card.setOrientation(LinearLayout.HORIZONTAL);
        card.setGravity(Gravity.CENTER_VERTICAL);
        card.setBackground(Ui.rounded(Ui.accentSoft(context), Ui.dp(context, 20)));
        int pad = Ui.dp(context, 16);
        card.setPadding(pad, pad, pad, pad);

        FrameLayout tile = new FrameLayout(context);
        tile.setBackground(Ui.rounded(Ui.accent(context), Ui.dp(context, 12)));
        ImageView icon = new ImageView(context);
        icon.setImageResource(R.drawable.ic_ms_link);
        icon.setImageTintList(ColorStateList.valueOf(Ui.onAccent(context)));
        icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        int iconSize = Ui.dp(context, 24);
        tile.addView(icon, new FrameLayout.LayoutParams(iconSize, iconSize, Gravity.CENTER));
        int tileSize = Ui.dp(context, 44);
        card.addView(tile, new LinearLayout.LayoutParams(tileSize, tileSize));

        LinearLayout texts = new LinearLayout(context);
        texts.setOrientation(LinearLayout.VERTICAL);
        TextView title = new TextView(context);
        title.setText("在电脑上打开");
        Ui.style(title, Ui.TEXT_ROW_TITLE, 600, Ui.text(context));
        texts.addView(title);
        TextView link = new TextView(context);
        link.setText(DOWNLOAD_LABEL);
        Ui.style(link, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
        texts.addView(link);
        LinearLayout.LayoutParams textParams = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        textParams.setMarginStart(Ui.dp(context, 14));
        card.addView(texts, textParams);

        TextView copy = new TextView(context);
        copy.setText("复制链接");
        copy.setGravity(Gravity.CENTER);
        copy.setSingleLine(true);
        Ui.style(copy, Ui.TEXT_BUTTON_SMALL, 600, Ui.onAccent(context));
        copy.setBackground(Ui.rippleOn(context, Ui.accent(context), 9999f));
        copy.setPadding(Ui.dp(context, 14), Ui.dp(context, 6), Ui.dp(context, 14), Ui.dp(context, 6));
        copy.setMinHeight(Ui.dp(context, 32));
        copy.setClickable(true);
        copy.setFocusable(true);
        copy.setContentDescription("复制下载页链接");
        copy.setOnClickListener(ignored -> copyLink(context));
        LinearLayout.LayoutParams copyParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        copyParams.setMarginStart(Ui.dp(context, 12));
        card.addView(copy, copyParams);
        return card;
    }

    private static void copyLink(Context context) {
        ClipboardManager clipboard = context.getSystemService(ClipboardManager.class);
        if (clipboard == null) return;
        clipboard.setPrimaryClip(ClipData.newPlainText("水杉下载页", DOWNLOAD));
        MsToast.show(context, "链接已复制");
    }

    /** 带图标的值行，行尾留给调用方放按钮或文字。 */
    private static GroupCard.Row row(GroupCard group, @DrawableRes int icon, String title, String subtitle) {
        GroupCard.Row row = group.value(title, subtitle, null);
        Context context = row.view().getContext();
        ImageView image = new ImageView(context);
        image.setImageResource(icon);
        image.setImageTintList(ColorStateList.valueOf(Ui.text(context)));
        image.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        int size = Ui.dp(context, 24);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(size, size);
        params.setMarginEnd(Ui.dp(context, 18));
        ((LinearLayout) row.view()).addView(image, 0, params);
        return row;
    }

    private void getRow(GroupCard group, @DrawableRes int icon, String title, String subtitle, String platform) {
        GroupCard.Row row = row(group, icon, title, subtitle);
        Context context = row.view().getContext();
        TextView button = tonal(context, "获取", title);
        button.setOnClickListener(ignored -> AboutPage.openLink(context, DOWNLOAD + "?release=" + platform));
        attach(row, button);
    }

    private void sendRow(GroupCard group, @DrawableRes int icon, String title, String subtitle, String platform) {
        GroupCard.Row row = row(group, icon, title, subtitle);
        Context context = row.view().getContext();
        TextView button = tonal(context, sentPlatforms.contains(platform) ? "已发送" : "发送链接", title);
        button.setEnabled(!sentPlatforms.contains(platform));
        button.setOnClickListener(ignored -> send(platform, button));
        attach(row, button);
    }

    private static void attach(GroupCard.Row row, TextView button) {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.setMarginStart(Ui.dp(button.getContext(), Ui.ROW_GAP));
        ((LinearLayout) row.view()).addView(button, params);
    }

    /** 行尾的 tonal 胶囊：accentSoft 底、强调色字。 */
    private static TextView tonal(Context context, String label, String title) {
        TextView button = new TextView(context);
        button.setText(label);
        button.setGravity(Gravity.CENTER);
        button.setSingleLine(true);
        Ui.style(button, Ui.TEXT_BUTTON_SMALL, 600, Ui.accent(context));
        button.setBackground(Ui.rippleOn(context, Ui.accentSoft(context), 9999f));
        button.setPadding(Ui.dp(context, 14), Ui.dp(context, 5), Ui.dp(context, 14), Ui.dp(context, 5));
        button.setMinHeight(Ui.dp(context, 32));
        button.setClickable(true);
        button.setFocusable(true);
        button.setContentDescription(label + "，" + title);
        button.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        return button;
    }

    private void send(String platform, TextView button) {
        if (sending.contains(platform) || sentPlatforms.contains(platform)) return;
        Context application = requireContext().getApplicationContext();
        sending.add(platform);
        button.setText("正在发送…");
        button.setEnabled(false);
        AboutPage.network(this, () -> new DownloadLinkApi(new CloudApi(application)).send(platform), outcome -> {
            sending.remove(platform);
            if (outcome.error() != null) {
                button.setText("发送链接");
                button.setEnabled(true);
                MsToast.show(requireContext(), failureMessage(outcome.error()));
                return;
            }
            sentPlatforms.add(platform);
            button.setText("已发送");
            String to = outcome.value();
            MsToast.show(requireContext(), to == null || to.isEmpty() ? "链接已发送到你的邮箱" : "链接已发送到 " + to);
        });
    }

    private static String failureMessage(@Nullable Exception error) {
        if (error instanceof CloudApi.Failure failure) {
            if (DownloadLinkApi.noVerifiedEmail(failure)) return "账号还没有验证过的邮箱，请改用「复制链接」";
            if (failure.signedOut()) return "登录后才能发到邮箱，也可以改用「复制链接」";
            if (failure.status == 429) return "发送得太频繁了，请过一会儿再试";
            if (failure.unavailable()) return "暂时不能发送邮件，请改用「复制链接」";
            if (failure.network()) return "连不上服务器，请检查网络后重试";
        }
        return "没有发送成功，请改用「复制链接」";
    }
}
