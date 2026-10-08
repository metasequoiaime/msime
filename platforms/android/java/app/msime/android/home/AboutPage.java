package app.msime.android.home;

import android.Manifest;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.res.AssetManager;
import android.net.Uri;
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
import androidx.fragment.app.Fragment;
import app.msime.android.AppEdition;
import app.msime.android.BoundsPolicy;
import app.msime.android.DeviceInfoReport;
import app.msime.android.ViewPolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.R;
import app.msime.android.ResourcePacks;
import app.msime.android.UpdateApi;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.LinkOption;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.function.Consumer;

/**
 * 关于：居中的标、应用名与版本、检查更新药丸，更新设置（自动更新、更新通道），设备信息（反馈问题时一键复制，{@link DeviceInfo}），官网、隐私政策、开源许可等链接，页脚一行版权与「输入内容默认只在本机处理」。
 *
 * <p>检查更新查 msime.app 的 Android 发行版，找到新版本后由用户点「下载」，下载完核对 SHA-256 与签名证书再交给系统安装器（{@link UpdateApi}）。从 Google Play 安装时这三项（检查更新、自动更新、更新通道）都不显示，Play 的政策不允许应用自己更新；同时「给我们评分」才有确定的去处，所以只在这种情况下显示。用户协议还没有确定的页面，不显示。
 *
 * <p>开源许可列出 APK 里随包带的许可通知（`assets/native-notices/`、词库与离线释义的许可等），以及已下载的资源包（日文词典、语言词库、离线释义）里随数据一起下载的许可文本，都没有时（例如开发构建）链接到仓库。
 */
public final class AboutPage extends DetailPage {
    private static final String SITE = "https://msime.app/";
    private static final String PRIVACY = "https://msime.app/privacy/";
    private static final String REPOSITORY = "https://github.com/metasequoiaime/msime";
    private static final int MAX_NOTICE_CHARS = 200_000;
    /** 按需开线程：几十兆的更新下载不能让云剪贴板、反馈这些短请求排在它后面。 */
    private static final ExecutorService NETWORK = Executors.newCachedThreadPool(runnable -> {
        Thread thread = new Thread(runnable, "msime-home-network");
        thread.setDaemon(true);
        return thread;
    });
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

    private final ActivityResultLauncher<String> notificationPermission =
        registerForActivityResult(new ActivityResultContracts.RequestPermission(), granted -> {});

    /** 一次后台调用的结果：成功时 `value` 有值，失败时 `error` 有值。 */
    record Outcome<T>(@Nullable T value, @Nullable Exception error) {}

    /**
     * 在宿主共用的网络线程上跑一次阻塞调用，回到主线程交结果；页面已经离开时丢掉结果。
     *
     * <p>不用 {@link HostTask}：那条线程还要处理共享存储的读写，一次几十兆的下载不能把它们都堵住。返回的 Future 可以用来在离开页面时取消；页面没有视图时不提交，返回 null。
     */
    @Nullable
    static <T> Future<?> network(Fragment fragment, Callable<T> work, Consumer<Outcome<T>> done) {
        View owner = fragment.getView();
        if (owner == null) return null;
        return NETWORK.submit(() -> {
            Outcome<T> outcome;
            try {
                outcome = new Outcome<>(work.call(), null);
            } catch (Exception error) {
                outcome = new Outcome<>(null, error);
            }
            Outcome<T> result = outcome;
            MAIN.post(() -> {
                if (fragment.isAdded() && fragment.getView() == owner) done.accept(result);
            });
        });
    }

    /** 用浏览器或别的应用打开一个链接；没有应用接得住时提示一句。 */
    static void openLink(Context context, String url) {
        try {
            context.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
        } catch (RuntimeException unavailable) {
            MsToast.show(context, "没有可以打开这个链接的应用");
        }
    }

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();
        boolean play = UpdateJobService.installedFromPlay(context);
        column.addView(header(context, play));

        if (!play) {
            GroupCard updates = GroupCard.add(column, "更新");
            updates.toggle("自动更新", "每天在联网时检查一次，下载好后通知你安装", UpdateJobService.autoUpdate(context),
                this::setAutoUpdate);
            channelRow = updates.nav("更新通道", null, UpdateJobService.channel(context).title(), this::chooseChannel);
        }

        GroupCard device = GroupCard.add(column, "设备信息").withDividers(Ui.ROW_PADDING_H);
        TextView reading = device.note("正在读取…");
        DeviceInfo.load(this, entries -> {
            ViewPolicy.hide(reading);
            for (DeviceInfoReport.Entry entry : entries) {
                device.value(entry.label(), DeviceInfoReport.valueOrUnknown(entry.value()), null);
            }
            device.button("复制设备信息", "反馈问题时贴进去，方便开发者排查", "复制",
                () -> ClipboardActions.copyText(context, "设备信息", DeviceInfo.text(context, entries), "已复制设备信息"));
        });

        GroupCard links = GroupCard.add(column, null).withDividers(Ui.ROW_PADDING_H);
        links.nav("官网", null, "msime.app", () -> openLink(context, SITE));
        links.nav("隐私政策", null, null, () -> openLink(context, PRIVACY));
        GroupCard.Row licences = links.nav("开源许可", null, null, this::showLicences);
        if (play) {
            links.nav("给我们评分", null, null, () -> openLink(context,
                "market://details?id=" + context.getPackageName()));
        }
        if (Ui.tauriAvailable()) {
            links.nav("在管理界面中查看", "更新日志、致谢与更多信息", null, this::openTauriAbout);
        }

        TextView footer = Ui.styledLabel(context, "© 2026 Metasequoia · 输入内容默认只在本机处理",
            13, 400, Ui.subText(context));
        ViewPolicy.setCentered(footer);
        LinearLayout.LayoutParams footerParams = Ui.matchWidth();
        footerParams.topMargin = Ui.dp(context, 24);
        column.addView(footer, footerParams);

        if (notices == null) {
            AssetManager assets = context.getAssets();
            Context application = context.getApplicationContext();
            network(this, () -> {
                List<String> found = new ArrayList<>(listNotices(assets));
                found.addAll(packNotices(application));
                return found;
            }, outcome -> {
                notices = outcome.value() == null ? List.of() : outcome.value();
                showNoticeCount(licences);
            });
        } else {
            showNoticeCount(licences);
        }
        renderPill();
    }

    @Override public void onDestroyView() {
        if (downloadTask != null) {
            downloadTask.cancel(true);
            downloadTask = null;
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
        LinearLayout header = Ui.column(context);
        ViewPolicy.setCenteredHorizontally(header);
        Ui.setPaddingDp(header, context, 0, 8, 0, 20);

        FrameLayout disc = new FrameLayout(context);
        ViewPolicy.setBackground(disc, Ui.pill(Ui.color(context, com.google.android.material.R.attr.colorTertiaryContainer)));
        ImageView mark = Ui.decorativeIcon(context, R.drawable.splash_mark);
        int markSize = Ui.dp(context, 60);
        disc.addView(mark, Ui.squareFrameParamsPx(markSize, Gravity.CENTER));
        int discSize = Ui.dp(context, 116);
        header.addView(disc, Ui.squareParamsPx(discSize));

        TextView name = Ui.styledLabel(context, getString(R.string.app_name), 22, 700, Ui.text(context));
        ViewPolicy.setCentered(name);
        name.setAccessibilityHeading(true);
        LinearLayout.LayoutParams nameParams = Ui.wrap();
        nameParams.topMargin = Ui.dp(context, 18);
        header.addView(name, nameParams);

        TextView version = Ui.styledLabel(context,
            "版本 " + UpdateJobService.currentVersion(context) + " · Android", 13, 400, Ui.subText(context));
        ViewPolicy.setCentered(version);
        LinearLayout.LayoutParams versionParams = Ui.wrap();
        versionParams.topMargin = Ui.dp(context, 6);
        header.addView(version, versionParams);

        if (!play) {
            TextView button = Ui.pillButton(context, "检查更新", 15, 600, Ui.onAccent(context),
                20, 0, 36, 96, this::onPill);
            ViewPolicy.setPoliteLiveRegion(button);
            LinearLayout.LayoutParams pillParams = Ui.wrap();
            pillParams.topMargin = Ui.dp(context, 14);
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
        ViewPolicy.setTextColor(button, quiet ? Ui.accent(context) : Ui.onAccent(context));
        ViewPolicy.setBackground(button, Ui.pillRipple(context, quiet ? Ui.accentSoft(context) : Ui.accent(context)));
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
        network(this, () -> new UpdateApi().check(channel, AppEdition.current().id(),
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
        Context context = requireContext().getApplicationContext();
        state = State.DOWNLOADING;
        percent = 0;
        renderPill();
        downloadTask = network(this, () -> {
            File apk = new UpdateApi().download(target, context.getCacheDir(), (done, total) -> {
                // 离开页面时 onDestroyView 中断了这条线程：停在这一块，不再下完。
                if (Thread.currentThread().isInterrupted())
                    throw new java.util.concurrent.CancellationException("update download cancelled");
                if (total <= 0) return;
                int value = (int) BoundsPolicy.atMost(done * 100 / total, 100L);
                MAIN.post(() -> {
                    if (value == percent || state != State.DOWNLOADING) return;
                    percent = value;
                    renderPill();
                });
            });
            try {
                UpdateApi.verifyArchive(context, apk);
            } catch (UpdateApi.Failure rejected) {
                if (!apk.delete()) apk.deleteOnExit();
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
        for (String fixed : new String[] {"client-LICENSE.txt", "helpcodes/NOTICE.md", "helpcodes/NOTICE-jiajia.md",
                "offline-glosses/offline-glosses-NOTICE.txt"}) {
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
        network(this, () -> {
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
            new MaterialAlertDialogBuilder(requireContext())
                .setTitle(noticeTitle(path))
                .setMessage(outcome.value())
                .setPositiveButton("好", null)
                .show();
        });
    }

    // ---- P21：只在 Tauri 合包下有用的入口 ----

    private void openTauriAbout() {
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "about");
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
