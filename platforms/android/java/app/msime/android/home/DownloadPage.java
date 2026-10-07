package app.msime.android.home;

import android.content.Context;
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
import app.msime.android.ViewPolicy;
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
    private final Set<String> sentPlatforms = new HashSet<>(6);
    private final Set<String> sending = new HashSet<>(6);

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
        TextView current = Ui.styledLabel(context, "当前设备", Ui.TEXT_BUTTON_SMALL, 500, Ui.text(context));
        LinearLayout.LayoutParams currentParams = Ui.rowGapParams(context);
        ((LinearLayout) android.view()).addView(current, currentParams);
        getRow(mobile, R.drawable.ic_ms_smartphone, "HarmonyOS", "从源码构建", "harmony");
    }

    /** 「在电脑上打开」卡：accentSoft 底的 r20 卡片，左边强调色圆角方块里一枚链接图标。 */
    private View hero(Context context) {
        LinearLayout card = Ui.row(context);
        ViewPolicy.setCenteredVertically(card);
        card.setBackground(Ui.rounded(Ui.accentSoft(context), Ui.dp(context, 20)));
        int pad = Ui.dp(context, 16);
        Ui.setSymmetricPaddingPx(card, pad);

        FrameLayout tile = new FrameLayout(context);
        tile.setBackground(Ui.rounded(Ui.accent(context), Ui.dp(context, 12)));
        ImageView icon = Ui.decorativeIcon(context, R.drawable.ic_ms_link, Ui.onAccent(context));
        int iconSize = Ui.dp(context, 24);
        tile.addView(icon, Ui.squareFrameParamsPx(iconSize, Gravity.CENTER));
        int tileSize = Ui.dp(context, 44);
        card.addView(tile, Ui.squareParamsPx(tileSize));

        LinearLayout texts = Ui.column(context);
        TextView title = Ui.styledLabel(context, "在电脑上打开", Ui.TEXT_ROW_TITLE, 600, Ui.text(context));
        texts.addView(title);
        TextView link = Ui.styledLabel(context, DOWNLOAD_LABEL, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
        texts.addView(link);
        LinearLayout.LayoutParams textParams = Ui.weightWrap(1f);
        textParams.setMarginStart(Ui.dp(context, 14));
        card.addView(texts, textParams);

        TextView copy = Ui.pillButton(context, "复制链接", Ui.TEXT_BUTTON_SMALL, 600, Ui.onAccent(context),
            14, 6, Ui.COMPACT_BUTTON_MIN_HEIGHT, 0, () -> copyLink(context));
        copy.setContentDescription("复制下载页链接");
        LinearLayout.LayoutParams copyParams = Ui.wrap();
        copyParams.setMarginStart(Ui.dp(context, 12));
        card.addView(copy, copyParams);
        return card;
    }

    private static void copyLink(Context context) {
        ClipboardActions.copyText(context, "水杉下载页", DOWNLOAD, "链接已复制");
    }

    /** 带图标的值行，行尾留给调用方放按钮或文字。 */
    private static GroupCard.Row row(GroupCard group, @DrawableRes int icon, String title, String subtitle) {
        GroupCard.Row row = group.value(title, subtitle, null);
        Context context = row.view().getContext();
        ImageView image = Ui.decorativeIcon(context, icon, Ui.text(context));
        int size = Ui.dp(context, 24);
        LinearLayout.LayoutParams params = Ui.squareParamsPx(size);
        params.setMarginEnd(Ui.dp(context, 18));
        ((LinearLayout) row.view()).addView(image, 0, params);
        return row;
    }

    private void getRow(GroupCard group, @DrawableRes int icon, String title, String subtitle, String platform) {
        GroupCard.Row row = row(group, icon, title, subtitle);
        Context context = row.view().getContext();
        TextView button = KeyboardSheets.tonalButton(context, "获取", "获取，" + title, 600,
            () -> AboutPage.openLink(context, DOWNLOAD + "?release=" + platform));
        attach(row, button);
    }

    private void sendRow(GroupCard group, @DrawableRes int icon, String title, String subtitle, String platform) {
        GroupCard.Row row = row(group, icon, title, subtitle);
        Context context = row.view().getContext();
        String label = sentPlatforms.contains(platform) ? "已发送" : "发送链接";
        TextView button = KeyboardSheets.tonalButton(context, label, label + "，" + title, 600);
        ViewPolicy.setEnabled(button, !sentPlatforms.contains(platform));
        ViewPolicy.bindClick(button, () -> send(platform, button));
        attach(row, button);
    }

    private static void attach(GroupCard.Row row, TextView button) {
        LinearLayout.LayoutParams params = Ui.rowGapParams(button.getContext());
        ((LinearLayout) row.view()).addView(button, params);
    }

    private void send(String platform, TextView button) {
        if (sending.contains(platform) || sentPlatforms.contains(platform)) return;
        Context application = requireContext().getApplicationContext();
        sending.add(platform);
        button.setText("正在发送…");
        ViewPolicy.setEnabled(button, false);
        AboutPage.network(this, () -> new DownloadLinkApi(new CloudApi(application)).send(platform), outcome -> {
            sending.remove(platform);
            if (outcome.error() != null) {
                button.setText("发送链接");
                ViewPolicy.setEnabled(button, true);
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
