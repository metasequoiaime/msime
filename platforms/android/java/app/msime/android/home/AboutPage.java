package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.ImageViewPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ThemeColorPolicy;
import android.Manifest;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.res.AssetManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.AppVersionPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.DeviceInfoReport;
import app.msime.android.FilePolicy;
import app.msime.android.ListPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.R;
import app.msime.android.ResourcePacks;
import app.msime.android.UpdateApi;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.LinkOption;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Future;

/**
 * 关于：居中的标、应用名与版本、检查更新药丸，更新设置（自动更新、更新通道），设备信息（反馈问题时一键复制，{@link DeviceInfo}），官网、隐私政策、开源许可等链接，页脚一行版权与「输入内容默认只在本机处理」。
 *
 * <p>检查更新查 msime.app 的 Android 发行版，找到新版本后由用户点「下载」，下载完核对 SHA-256 与签名证书再交给系统安装器（{@link UpdateApi}）。由 Google Play 负责更新时（{@link UpdateJobService#updatedByPlay}）这三项（检查更新、自动更新、更新通道）都不显示，Play 的政策不允许应用自己更新；同时「给我们评分」才有确定的去处，所以只在这种情况下显示。用户协议还没有确定的页面，不显示。
 *
 * <p>开源许可列出 APK 里随包带的许可通知（`assets/native-notices/`、词库与离线释义的许可等），以及已下载的资源包（日文词典、语言词库、离线释义）里随数据一起下载的许可文本，都没有时（例如开发构建）链接到仓库。
 */
public final class AboutPage extends DetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("应用", "权限", "设备", "链接");
    }

    private static final String SITE = "https://msime.app/";
    private static final String PRIVACY = "https://msime.app/privacy/";
    private static final String REPOSITORY = "https://github.com/metasequoiaime/msime";
    private static final int MAX_NOTICE_CHARS = 200_000;
    private static final Handler MAIN = new Handler(Looper.getMainLooper());

    /** 检查更新药丸的几种状态。 */
    private enum State { IDLE, CHECKING, UP_TO_DATE, AVAILABLE, DOWNLOADING, READY }

    private State state = State.IDLE;
    @Nullable private UpdateApi.Update update;
    @Nullable private File downloaded;
    private int percent;
    @Nullable private TextView pill;
    @Nullable private GroupCard.Row channelRow;
    @Nullable private List<String> notices;
    /** 正在进行的下载；离开页面时取消，不在后台继续下完再把结果丢掉。 */
    @Nullable private Future<?> downloadTask;
    /** 视图重建后递增，阻止旧下载的进度回调写入新页面。 */
    private long viewGeneration;

    private final ActivityResultLauncher<String> notificationPermission =
        registerForActivityResult(new ActivityResultContracts.RequestPermission(), granted -> {});

    /** 用浏览器或别的应用打开一个链接；没有应用接得住时提示一句。 */
    static void openLink(Context context, String url) {
        if (!LinkActions.open(context, url)) {
            MsToast.show(context, "没有可以打开这个链接的应用");
        }
    }

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();
        boolean play = UpdateJobService.updatedByPlay(context);
        sectionColumn(column, "应用").addView(header(context, play));

        if (!play) {
            GroupCard updates = GroupCard.add(sectionColumn(column, "应用"), "更新");
            updates.toggle("自动更新", null, UpdateJobService.autoUpdate(context),
                this::setAutoUpdate).help("每天在联网时检查一次，下载好后通知你安装");
            channelRow = updates.nav("更新通道", null, UpdateJobService.channel(context).title(), this::chooseChannel);
        }

        GroupCard permissions = GroupCard.add(sectionColumn(column, "权限"), "权限说明");
        permissions.note("录音：用于语音输入和本地语音识别；关闭后不能使用语音功能。\n\n"
            + "相机：用于拍照识字；关闭后不能使用拍照识字。\n\n"
            + "联系人：只在你点击导入联系人时读取电话本；关闭后不能导入联系人，已导入的名单不受影响。\n\n"
            + "网络：用于连接社区、下载皮肤和词库、使用在线 AI 与同步服务；关闭后这些联网功能不可用，离线输入不受影响。\n\n"
            + "通知：用于提醒下载完成和应用更新；关闭后不会收到这些通知。\n\n"
            + "震动：用于按键反馈；关闭后键盘仍可输入，只是不震动。权限均可在系统设置中重新开启或撤回。");

        GroupCard device = GroupCard.add(sectionColumn(column, "设备"), "设备信息").withDividers(Ui.ROW_PADDING_H);
        TextView reading = device.note("正在读取…");
        DeviceInfo.load(this, entries -> {
            ViewPolicy.hide(reading);
            for (DeviceInfoReport.Entry entry : entries) {
                device.value(entry.label(), DeviceInfoReport.valueOrUnknown(entry.value()), null);
            }
            device.button("复制设备信息", null, "复制",
                () -> ClipboardActions.copyText(context, "设备信息", DeviceInfo.text(context, entries), "已复制设备信息")).help("反馈问题时贴进去，方便开发者排查");
        });

        GroupCard links = GroupCard.add(sectionColumn(column, "链接"), null).withDividers(Ui.ROW_PADDING_H);
        links.nav("官网", null, "msime.app", () -> openLink(context, SITE));
        links.nav("项目主页", null, "github.com/metasequoiaime/msime", () -> openLink(context, REPOSITORY));
        links.nav("隐私政策", null, null, () -> openLink(context, PRIVACY));
        GroupCard.Row licences = links.nav("开源许可", null, null, this::showLicences);
        if (play) {
            links.nav("给我们评分", null, null, () -> openLink(context,
                "market://details?id=" + context.getPackageName()));
        }
        if (ManagementUi.available()) {
            links.nav("在管理界面中查看", null, null, this::openTauriAbout).help("更新日志、致谢与更多信息");
        }
        links.nav("开发者选项", null, null,
            () -> SettingsNavigator.open(context, PageId.DEVELOPER, null)).help("日志、诊断包和开发者访问");

        TextView footer = ViewPolicy.styledLabel(context, "© 2026 Metasequoia · 输入内容默认只在本机处理",
            13, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setCentered(footer);
        LinearLayout.LayoutParams footerParams = LayoutPolicy.matchWidthWrapParams();
        footerParams.topMargin = DimensionPolicy.pixels(context, 24);
        sectionColumn(column, "链接").addView(footer, footerParams);

        if (notices == null) {
            AssetManager assets = context.getAssets();
            Context application = context.getApplicationContext();
            NetworkTask.run(this, () -> {
                List<String> found = new ArrayList<>(listNotices(assets));
                found.addAll(packNotices(application));
                return found;
            }, outcome -> {
                notices = ListPolicy.copyOrEmpty(outcome.value());
                showNoticeCount(licences);
            });
        } else {
            showNoticeCount(licences);
        }
        renderPill();
    }

    @Override public void onDestroyView() {
        viewGeneration++;
        if (downloadTask != null) {
            downloadTask.cancel(true);
            downloadTask = null;
        }
        // The network result is fenced to this view, so an abandoned check or download cannot
        // report back after recreation. Reset its transient state or the rebuilt page would show
        // a permanently disabled busy pill with no task left to finish it.
        if (state == State.CHECKING || state == State.DOWNLOADING) {
            state = State.IDLE;
            downloaded = null;
        }
        pill = null;
        channelRow = null;
        super.onDestroyView();
    }

    private void showNoticeCount(GroupCard.Row row) {
        if (notices != null && !notices.isEmpty()) row.setValue(notices.size() + " 个组件");
    }

    /** 居中的标、应用名、版本，以及检查更新药丸（Play 安装时没有）。 */
    private View header(Context context, boolean play) {
        LinearLayout header = LayoutPolicy.column(context);
        ViewPolicy.setCenteredHorizontally(header);
        ViewPolicy.setPaddingDp(context, header, 0, 8, 0, 20);

        FrameLayout disc = new FrameLayout(context);
        ViewPolicy.setBackground(disc, DrawablePolicy.pill(ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorTertiaryContainer)));
        ImageView mark = ImageViewPolicy.decorative(context, R.drawable.splash_mark);
        int markSize = DimensionPolicy.pixels(context, 60);
        disc.addView(mark, LayoutPolicy.squareFrameParamsPx(markSize, Gravity.CENTER));
        int discSize = DimensionPolicy.pixels(context, 116);
        header.addView(disc, ViewPolicy.newSquareParamsPx(discSize));

        TextView name = ViewPolicy.styledLabel(context, getString(R.string.app_name), 22, 700, ThemeColorPolicy.text(context));
        ViewPolicy.setCentered(name);
        name.setAccessibilityHeading(true);
        LinearLayout.LayoutParams nameParams = LayoutPolicy.wrapParams();
        nameParams.topMargin = DimensionPolicy.pixels(context, 18);
        header.addView(name, nameParams);

        TextView version = ViewPolicy.styledLabel(context,
            "版本 " + AppVersionPolicy.displayVersion(UpdateJobService.currentVersion(context), AppVersionPolicy.releaseDate())
                + " · Android", 13, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setCentered(version);
        LinearLayout.LayoutParams versionParams = LayoutPolicy.wrapParams();
        versionParams.topMargin = DimensionPolicy.pixels(context, 6);
        header.addView(version, versionParams);

        if (!play) {
            TextView button = ViewPolicy.pillButton(context, "检查更新", 15, 600, ThemeColorPolicy.accent(context),
                ThemeColorPolicy.onAccent(context),
                20, 0, 36, 96, this::onPill);
            ViewPolicy.setPoliteLiveRegion(button);
            LinearLayout.LayoutParams pillParams = LayoutPolicy.wrapParams();
            pillParams.topMargin = DimensionPolicy.pixels(context, 14);
            header.addView(button, pillParams);
            pill = button;
        }
        return header;
    }

    private void renderPill() {
        TextView button = pill;
        if (button == null) return;
        Context context = button.getContext();
        String version = update == null ? "" : " " + update.release().version();
        switch (state) {
            case CHECKING -> button.setText("正在检查…");
            case UP_TO_DATE -> button.setText("✓ 已是最新版本");
            case AVAILABLE -> button.setText("下载" + version);
            case DOWNLOADING -> button.setText("正在下载 " + percent + "%");
            case READY -> button.setText("安装" + version);
            default -> button.setText("检查更新");
        }
        boolean busy = state == State.CHECKING || state == State.DOWNLOADING;
        ViewPolicy.setEnabled(button, !busy);
        // 「已是最新版本」是结果而不是按钮，换成 accentSoft 底、强调色字，再点一次重新检查。
        boolean quiet = state == State.UP_TO_DATE || busy;
        ViewPolicy.setTextColor(button, quiet ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.onAccent(context));
        ViewPolicy.setBackground(button, ViewPolicy.pillRipple(context, quiet ? ThemeColorPolicy.accentSoft(context) : ThemeColorPolicy.accent(context)));
    }

    private void onPill() {
        switch (state) {
            case AVAILABLE -> download();
            case READY -> install();
            case CHECKING, DOWNLOADING -> { }
            default -> check();
        }
    }

    private void check() {
        Context context = requireContext().getApplicationContext();
        state = State.CHECKING;
        renderPill();
        UpdateApi.Channel channel = UpdateJobService.channel(context);
        NetworkTask.run(this, () -> new UpdateApi().check(channel, AppEdition.current().id(),
            UpdateJobService.currentVersion(context)), outcome -> {
            if (outcome.error() != null) {
                state = State.IDLE;
                renderPill();
                MsToast.show(requireContext(), message(outcome.error()));
                return;
            }
            update = outcome.value();
            state = update == null ? State.UP_TO_DATE : State.AVAILABLE;
            renderPill();
        });
    }

    private void download() {
        UpdateApi.Update target = update;
        if (target == null) return;
        View owner = getView();
        if (owner == null) return;
        long viewToken = viewGeneration;
        Context context = requireContext().getApplicationContext();
        state = State.DOWNLOADING;
        percent = 0;
        renderPill();
        downloadTask = NetworkTask.run(this, () -> {
            File apk = new UpdateApi().download(target, context.getCacheDir(), (done, total) -> {
                // 离开页面时 onDestroyView 中断了这条线程：停在这一块，不再下完。
                if (Thread.currentThread().isInterrupted())
                    throw new java.util.concurrent.CancellationException("update download cancelled");
                if (total <= 0) return;
                int value = (int) BoundsPolicy.atMost(done * 100 / total, 100L);
                MAIN.post(() -> {
                    if (getView() != owner || viewGeneration != viewToken
                            || value == percent || state != State.DOWNLOADING) return;
                    percent = value;
                    renderPill();
                });
            });
            try {
                UpdateApi.verifyArchive(context, apk);
            } catch (UpdateApi.Failure rejected) {
                FilePolicy.deleteQuietly(apk);
                throw rejected;
            }
            return apk;
        }, outcome -> {
            downloadTask = null;
            if (outcome.error() != null) {
                state = State.AVAILABLE;
                renderPill();
                MsToast.show(requireContext(), message(outcome.error()));
                return;
            }
            downloaded = outcome.value();
            state = State.READY;
            renderPill();
            install();
        });
    }

    private void install() {
        Context context = requireContext();
        File apk = downloaded;
        if (apk == null || !apk.isFile()) {
            state = update == null ? State.IDLE : State.AVAILABLE;
            renderPill();
            return;
        }
        try {
            if (!context.getPackageManager().canRequestPackageInstalls()) {
                MsToast.show(context, "请允许安装未知应用，返回后再点一次「安装」");
                startActivity(UpdateJobService.unknownSourcesIntent(context));
                return;
            }
            startActivity(UpdateJobService.installIntent(context, apk));
        } catch (RuntimeException unavailable) {
            MsToast.show(context, "打不开系统安装器");
        }
    }

    private static String message(Exception error) {
        if (error instanceof UpdateApi.Failure failure && failure.getMessage() != null) return failure.getMessage();
        return "检查更新没有完成，请稍后再试";
    }

    private void setAutoUpdate(boolean enabled) {
        Context context = requireContext();
        UpdateJobService.setAutoUpdate(context, enabled);
        if (enabled && Build.VERSION.SDK_INT >= 33
                && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS);
        }
    }

    private void chooseChannel() {
        Context context = requireContext();
        UpdateApi.Channel current = UpdateJobService.channel(context);
        OptionSheet sheet = new OptionSheet(context, "更新通道", "预览版更早拿到新功能，也可能不太稳定");
        for (UpdateApi.Channel channel : UpdateApi.Channel.values()) {
            sheet.option(channel.title(), channel == current, () -> {
                UpdateJobService.setChannel(context, channel);
                if (channelRow != null) channelRow.setValue(channel.title());
                if (channel != current) {
                    update = null;
                    state = State.IDLE;
                    renderPill();
                }
            });
        }
        sheet.show();
    }

    // ---- 开源许可 ----

    /** APK 里随包带的许可通知文件（相对 assets 的路径），按名字排序。 */
    private static List<String> listNotices(AssetManager assets) throws IOException {
        String[] abis = assets.list("native-notices");
        List<String> found = new ArrayList<>(4);
        if (abis != null && abis.length > 0) {
            // 每个 ABI 一份同样的清单，列一份就够。
            java.util.Arrays.sort(abis);
            String[] files = assets.list("native-notices/" + abis[0]);
            if (files != null) {
                found = new ArrayList<>(files.length);
                for (String file : files) found.add("native-notices/" + abis[0] + "/" + file);
            }
        }
        for (String fixed : new String[] {"client-LICENSE.txt", "emoji-combinations-NOTICE.txt", "helpcodes/NOTICE.md", "helpcodes/NOTICE-jiajia.md",
                "helpcodes/NOTICE-wubi86.md", "offline-glosses/offline-glosses-NOTICE.txt",
                // 背单词内置词书取自 ECDICT（MIT），许可声明随词书一起打进 assets/wordbooks。
                "wordbooks/wordbooks-NOTICE.txt"}) {
            if (exists(assets, fixed)) found.add(fixed);
        }
        String[] languages = assets.list("language-dictionaries");
        if (languages != null) {
            for (String file : languages) {
                String upper = file.toUpperCase(java.util.Locale.ROOT);
                if (upper.contains("LICENSE") || upper.contains("LICENCE") || upper.contains("NOTICE")) {
                    found.add("language-dictionaries/" + file);
                }
            }
        }
        found.sort(String::compareTo);
        return found;
    }

    /**
     * 已下载的资源包里的许可文本（绝对路径）。精简安装包不再随包带日文词典、语言词库和离线释义，它们的许可文本跟着资源包下载到 `files/bootstrap/state/resource-packs/<id>/`。在工作线程上调用：要经共享层列出一次安装状态。
     */
    private static List<String> packNotices(Context context) {
        File packs = new File(ResourcePacks.stateRoot(context.getFilesDir()), "resource-packs");
        List<String> found = new ArrayList<>(8);
        for (String id : ResourcePacks.installedIds(context)) {
            File[] files = new File(packs, id).listFiles();
            for (File file : files == null ? new File[0] : files) {
                String upper = file.getName().toUpperCase(java.util.Locale.ROOT);
                if ((upper.contains("LICENSE") || upper.contains("LICENCE") || upper.contains("NOTICE")
                        || upper.contains("README"))
                        && java.nio.file.Files.isRegularFile(file.toPath(), LinkOption.NOFOLLOW_LINKS))
                    found.add(file.getAbsolutePath());
            }
        }
        found.sort(String::compareTo);
        return found;
    }

    private static boolean exists(AssetManager assets, String path) {
        try (InputStream ignored = assets.open(path)) {
            return true;
        } catch (IOException missing) {
            return false;
        }
    }

    /** 列表里显示的名字：文件名去掉扩展名。 */
    private String noticeTitle(String path) {
        String name = path.substring(path.lastIndexOf('/') + 1);
        if ("client-LICENSE.txt".equals(name)) return getString(R.string.app_name);
        int dot = name.lastIndexOf('.');
        return dot > 0 ? name.substring(0, dot) : name;
    }

    private void showLicences() {
        Context context = requireContext();
        List<String> list = notices;
        if (list == null || list.isEmpty()) {
            openLink(context, REPOSITORY);
            return;
        }
        OptionSheet sheet = new OptionSheet(context, "开源许可", "随安装包和已下载资源附带的许可通知");
        for (String path : list) sheet.option(noticeTitle(path), false, () -> showNotice(path));
        sheet.show();
    }

    private void showNotice(String path) {
        AssetManager assets = requireContext().getAssets();
        NetworkTask.run(this, () -> {
            // 绝对路径是已下载资源包里的许可文本，其余是 APK assets 里的。
            try (InputStream in = path.startsWith("/")
                    ? java.nio.file.Files.newInputStream(new File(path).toPath(), LinkOption.NOFOLLOW_LINKS)
                    : assets.open(path)) {
                byte[] bytes = HttpBodyPolicy.readBounded(in, MAX_NOTICE_CHARS * 4);
                if (bytes == null) return null;
                String text = new String(bytes, java.nio.charset.StandardCharsets.UTF_8);
                return TextPolicy.clipWithEllipsis(text, MAX_NOTICE_CHARS);
            }
        }, outcome -> {
            if (outcome.value() == null) {
                MsToast.show(requireContext(), "读不出这份许可");
                return;
            }
            MessageDialog.show(requireContext(), noticeTitle(path), outcome.value());
        });
    }

    // ---- P21：只在 Tauri 合包下有用的入口 ----

    private void openTauriAbout() {
        Intent intent = ManagementUi.settingsPage(requireContext(), "about");
        try {
            startActivity(intent);
        } catch (RuntimeException unavailable) {
            MsToast.show(requireContext(), "管理界面没有打开");
        }
    }

    @Override protected void onBecameVisible() {
        // 从系统的「安装未知应用」授权页回来时，药丸上的「安装」仍然有效；安装包被系统清掉了就退回「下载」。
        if (state == State.READY && (downloaded == null || !downloaded.isFile())) {
            state = update == null ? State.IDLE : State.AVAILABLE;
            renderPill();
        }
    }
}
