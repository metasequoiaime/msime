package app.msime.android.home;

import app.msime.android.TextPolicy;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.ImageViewPolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.text.InputFilter;
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
import androidx.core.content.FileProvider;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountNamePolicy;
import app.msime.android.AccountStore;
import app.msime.android.BitmapPolicy;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import app.msime.android.DrawablePolicy;
import app.msime.android.FilePolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.ListPolicy;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardOpenOption;
import java.time.LocalDate;
import java.time.ZoneId;
import java.util.List;
import java.util.UUID;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * 个人资料：头像（点了换一张）、昵称、水杉 ID、邮箱，三种登录方式的关联状态，云端数据（大小、导出、删除），退出登录与注销账号。
 *
 * <p>数据全部来自 {@link DeviceDataApi}，经 Rust 的账号入口（#8201 迁移计划 5c），页面绑定打开时的账号编号。注销账号和删除云端数据要求最近登录：收到 {@link DeviceDataApi.RecentLoginRequired} 时弹登录面板，用户重新登录同一个账号后自动重试一次；登录成了别的账号时不重试，免得删错。关联登录方式走登录面板的 `link` 用途，回来后按关联列表的变化说出关联了哪一种。
 *
 * <p>「退出登录」会撤销服务端的这次登录（`logout{all:false}`），断网时本机照样退出、提示服务端没有撤销。「删除云端数据」连同这台设备匿名账号名下的语音样本一起删；注销账号以后 Rust 也尽力删它们，那一步没做成时只提示样本没删掉。云端数据汇总合不进匿名账号的语音样本时，这一行写「语音样本数暂不可知」。
 */
public final class ProfilePage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("账号", "登录方式", "数据");
    }

    private static final String[] PROVIDERS = {"apple", "google", "email"};
    private static final String EXPORTS = "exports";
    private static final long EXPORT_RETENTION_MILLIS = 24L * 60 * 60 * 1000;
    /** 头像解码后的长边上限（像素）：最大的头像画 88dp，xxxhdpi 下约 352 像素，留一点余量。 */
    private static final int AVATAR_DECODE_EDGE = 384;
    /** 允许上传的头像长边上限（像素）：1 MiB 的平涂 PNG 可以有上万像素宽，解码会吃掉上 GB 内存。 */
    private static final int MAX_AVATAR_UPLOAD_EDGE = 8192;

    private final ActivityResultLauncher<String> picker =
        registerForActivityResult(new ActivityResultContracts.GetContent(), this::onPicked);
    @Nullable private Loaded loaded;
    @Nullable private String avatarPickerSessionId;
    private boolean busy;
    private long generation;

    /** 一次读取的结果：资料（未登录或读不到时为 null）、读不到的原因（Rust 的错误码，未登录是 account_unauthorized）、云端数据汇总（读不到时为 null）、后端开放的登录方式、头像、页面绑定的账号编号。 */
    private record Loaded(@Nullable DeviceDataApi.Profile profile, String failure,
            @Nullable DeviceDataApi.DataSummary data, CloudApi.Providers providers, @Nullable Bitmap avatar,
            String sessionId) {}

    @Override protected void onContentDestroyed() {
        generation++;
        loaded = null;
        avatarPickerSessionId = null;
        // Work results are fenced to the old view and will be dropped after recreation. Do not
        // leave the rebuilt page permanently disabled while an abandoned operation finishes.
        busy = false;
    }

    @Override protected void onContentReady() {
        loaded = null;
        render();
    }

    @Override protected boolean shouldReloadOnContentReady() { return false; }

    @Override protected void reload() { reload(null); }

    private void reload(@Nullable Consumer<Loaded> then) {
        long request = ++generation;
        HostTask.runNetwork(this, ProfilePage::load, result -> {
            if (result == null || request != generation || contentColumn() == null) return;
            loaded = result;
            render();
            if (then != null) then.accept(result);
        });
    }

    private static Loaded load(Context context) {
        CloudApi.Providers providers = SignIn.providers(context);
        String sessionId;
        try {
            sessionId = DeviceDataApi.currentUserId(context);
        } catch (AccountStore.Failure failure) {
            return new Loaded(null, failure.code, null, providers, null, "");
        }
        if (sessionId.isEmpty()) return new Loaded(null, "account_unauthorized", null, providers, null, "");
        DeviceDataApi api = new DeviceDataApi(context, sessionId);
        DeviceDataApi.Profile profile;
        try {
            profile = api.profile();
        } catch (AccountStore.Failure failure) {
            return new Loaded(null, failure.code, null, providers, null, "");
        }
        bindIfNeeded(context, profile, sessionId);
        DeviceDataApi.DataSummary data;
        try {
            data = api.dataSummary();
        } catch (AccountStore.Failure unavailable) {
            data = null;
        }
        return new Loaded(profile, "", data, providers, avatar(api, profile), sessionId);
    }

    /** 同步状态记着的账号与服务端说的不一致（登录时没读到用户 id，或换了账号）时重新绑定；绑定换账号时会关闭同步并清空游标。 */
    static void bindIfNeeded(Context context, DeviceDataApi.Profile profile, String expectedSessionId) {
        synchronized (SyncSwitch.bindingLock()) {
            if (expectedSessionId == null || expectedSessionId.isEmpty()
                    || !expectedSessionId.equals(new BackendAccount(context).sessionId())) return;
            String currentKind = SyncSwitch.loginKind(context);
            String kind = SyncSwitch.validLoginKind(currentKind) ? currentKind : profile.loginKind();
            if (kind.isEmpty()) return;
            String previousAccount = SyncSwitch.accountId(context);
            if (profile.id().equals(previousAccount) && kind.equals(currentKind)) return;
            if (!previousAccount.equals(profile.id())) {
                SignIn.cancelPendingSnapshot(context, previousAccount);
            }
            SyncSwitch.bindAccount(context, profile.id(), kind);
        }
    }

    /** 读头像图片：由 Rust 下载（只认账号头像服务的地址、不超过 1 MiB）；没有或读不到时为 null，界面显示首字头像。阻塞。 */
    @Nullable static Bitmap avatar(DeviceDataApi api, DeviceDataApi.Profile profile) {
        if (!profile.hasAvatar()) return null;
        try {
            byte[] bytes = api.avatar();
            return bytes == null ? null : decodeAvatar(bytes);
        } catch (AccountStore.Failure | RuntimeException unavailable) {
            return null;
        }
    }

    /** 按头像实际显示的大小降采样解码：只限字节数挡不住高度可压缩的大尺寸图片，全尺寸解码会 OutOfMemoryError。 */
    @Nullable private static Bitmap decodeAvatar(byte[] bytes) {
        BitmapFactory.Options bounds = BitmapPolicy.decodeBounds(bytes);
        if (bounds == null) return null;
        int sample = BitmapPolicy.sampleSizeForEdge(
            bounds.outWidth, bounds.outHeight, AVATAR_DECODE_EDGE);
        BitmapFactory.Options options = new BitmapFactory.Options();
        options.inSampleSize = sample;
        return BitmapFactory.decodeByteArray(bytes, 0, bytes.length, options);
    }

    /** 圆形头像：有图片时画图片，否则是强调色底上的昵称首字。 */
    static FrameLayout avatarView(Context context, int sizeDp, String name, @Nullable Bitmap image) {
        FrameLayout frame = new FrameLayout(context);
        GradientDrawable circle = DrawablePolicy.circle(ThemeColorPolicy.accent(context));
        if (image != null) {
            ImageView picture = new ImageView(context);
            picture.setImageBitmap(image);
            picture.setScaleType(ImageView.ScaleType.CENTER_CROP);
            ViewPolicy.setBackground(picture, circle);
            picture.setClipToOutline(true);
            ViewPolicy.hideFromAccessibility(picture);
            frame.addView(picture, LayoutPolicy.squareFrameParams(context, sizeDp));
        } else {
            TextView letter = ViewPolicy.centeredLabel(context, TextPolicy.trimmedInitial(name, "?"),
                Math.round(sizeDp * 0.4f), 600, ThemeColorPolicy.onAccent(context));
            ViewPolicy.setBackground(letter, circle);
            ViewPolicy.hideFromAccessibility(letter);
            frame.addView(letter, LayoutPolicy.squareFrameParams(context, sizeDp));
        }
        return frame;
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        Loaded state = loaded;
        if (state == null) {
            GroupCard.add(target, null).note("正在读取…");
            return;
        }
        if (state.profile() == null) {
            renderUnavailable(target, state.failure());
            return;
        }
        DeviceDataApi.Profile profile = state.profile();
        Context context = requireContext();
        sectionColumn(target, "账号").addView(header(context, profile, state.avatar()));

        GroupCard account = GroupCard.add(sectionColumn(target, "账号"), "账号").withDividers(16);
        account.nav("昵称", null, profile.displayName(), this::rename);
        account.nav("水杉 ID", null, profile.id(), () -> copy("水杉 ID", profile.id()));
        if (!profile.email().isEmpty()) account.value("邮箱", null, profile.email());

        GroupCard methods = GroupCard.add(sectionColumn(target, "登录方式"), "登录方式").withDividers(16);
        for (String provider : PROVIDERS) {
            boolean linked = profile.linked(provider);
            if (!linked && !offered(state.providers(), provider)) continue;
            methods.nav(profile.providerName(provider), null, linked ? "已关联" : "关联", linked ? null : this::link);
        }
        methods.help("关联后可以用任意一种方式登录同一个账号");

        GroupCard data = GroupCard.add(sectionColumn(target, "数据"), "数据").withDividers(16);
        data.nav("云端数据", state.data() == null ? null : AccountFailureText.anonymousVoice(state.data().anonymousVoice()),
            state.data() == null ? null : DeviceDataApi.formatBytes(state.data().bytes()), this::confirmDeleteData);
        data.nav("导出我的数据", null, null, this::export);

        GroupCard signOut = GroupCard.add(sectionColumn(target, "账号"), null);
        signOut.addView(dangerButton(context, "退出登录", this::confirmSignOut));
        GroupCard delete = GroupCard.add(sectionColumn(target, "账号"), null);
        delete.addView(dangerButton(context, "注销账号", this::confirmDeleteAccount));
        delete.footer("注销后云端的词库、皮肤、设置和社区作品会立即永久删除，无法恢复");
    }

    private void renderUnavailable(LinearLayout column, String failure) {
        GroupCard card = GroupCard.add(column, null);
        if ("account_unauthorized".equals(failure)) {
            card.note("登录后可以在这里修改昵称和头像、关联登录方式、导出或删除云端数据。");
            card.button("登录", null, "登录", this::signIn).help("Google、Apple 或邮箱");
        } else {
            // 读资料是已有的账号操作，断网时 Rust 报的也是 account_unavailable，分不出是网络还是服务。
            card.note("个人资料暂时读不到，请检查网络后再试。");
            card.button("重新读取", null, "重试", () -> reload(null));
        }
    }

    private View header(Context context, DeviceDataApi.Profile profile, @Nullable Bitmap image) {
        LinearLayout header = LayoutPolicy.column(context);
        ViewPolicy.setCenteredHorizontally(header);
        ViewPolicy.setPaddingDp(context, header, 0, 8, 0, 4);

        FrameLayout avatar = new FrameLayout(context);
        avatar.addView(avatarView(context, 88, profile.displayName(), image));
        ImageView camera = ImageViewPolicy.decorative(context, app.msime.android.R.drawable.ms_w5_me_camera,
            ThemeColorPolicy.text(context));
        GradientDrawable badge = DrawablePolicy.circle(ThemeColorPolicy.card(context));
        ViewPolicy.setBackground(camera, badge);
        int pad = DimensionPolicy.pixels(context, 6);
        ViewPolicy.setSymmetricPadding(camera, pad);
        ViewPolicy.hideFromAccessibility(camera);
        FrameLayout.LayoutParams cameraParams = LayoutPolicy.squareFrameParams(context, 28);
        cameraParams.gravity = Gravity.BOTTOM | Gravity.END;
        avatar.addView(camera, cameraParams);
        ViewPolicy.setInteractive(avatar, true);
        avatar.setContentDescription("更换头像");
        ViewPolicy.bindClick(avatar, this::chooseAvatar);
        header.addView(avatar, LayoutPolicy.squareParams(context, 92));

        TextView name = ViewPolicy.styledLabel(context, profile.displayName(), 22, 700, ThemeColorPolicy.text(context));
        LinearLayout.LayoutParams nameParams = LayoutPolicy.wrapParams();
        nameParams.topMargin = DimensionPolicy.pixels(context, 10);
        header.addView(name, nameParams);

        if (!profile.email().isEmpty()) {
            TextView email = ViewPolicy.styledLabel(context, profile.email(), Ui.TEXT_ROW_SUBTITLE, 400,
                ThemeColorPolicy.subText(context));
            LinearLayout.LayoutParams emailParams = LayoutPolicy.wrapParams();
            emailParams.topMargin = DimensionPolicy.pixels(context, 2);
            header.addView(email, emailParams);
        }

        String kind = SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)) ? SyncSwitch.loginKind(context)
            : profile.loginKind();
        if (!kind.isEmpty()) {
            TextView chip = ViewPolicy.styledLabel(context, "通过 " + profile.providerName(kind) + " 登录", 12, 500,
                ThemeColorPolicy.accent(context));
            ViewPolicy.setBackground(chip, DrawablePolicy.pill(ThemeColorPolicy.accentSoft(context)));
            ViewPolicy.setSymmetricPaddingDp(context, chip, 10, 3);
            LinearLayout.LayoutParams chipParams = LayoutPolicy.wrapParams();
            chipParams.topMargin = DimensionPolicy.pixels(context, 8);
            header.addView(chip, chipParams);
        }
        return header;
    }

    private static View dangerButton(Context context, CharSequence label, Runnable action) {
        TextView button = ViewPolicy.textButton(context, label, Ui.TEXT_ROW_TITLE, 500, ThemeColorPolicy.danger(context),
            ViewPolicy.ripple(context), Ui.ACTION_BUTTON_MIN_HEIGHT, action);
        return button;
    }

    private static boolean offered(CloudApi.Providers providers, String provider) {
        switch (provider) {
            case "apple": return providers.appleWeb();
            case "google": return providers.google();
            case "email": return providers.email();
            default: return false;
        }
    }

    // ---- 动作 ----

    private void rename() {
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        String expectedSessionId = state.sessionId();
        AccountNamePolicy.Presentation name = AccountNamePolicy.present(state.profile().displayName());
        if (name == null) {
            MsToast.show(requireContext(), "昵称校验暂不可用，请稍后重试");
            return;
        }
        InputDialog dialog = new InputDialog(requireContext(), "昵称", "1 到 " + name.maximumChars() + " 个字")
            .preserveWhitespace();
        dialog.addField("昵称", state.profile().displayName(), 0)
            .setFilters(new InputFilter[] {new InputFilter.LengthFilter(name.maximumUtf16Units())});
        dialog.setValidator(values -> {
            AccountNamePolicy.Presentation draft = AccountNamePolicy.present(values.get(0));
            return draft != null && draft.valid();
        });
        dialog.setPrimary("保存", values -> run(context -> {
            new DeviceDataApi(context, expectedSessionId).rename(values.get(0));
            return "";
        }, "昵称已更新"));
        dialog.show();
    }

    private void copy(String label, String value) {
        ClipboardActions.copyText(requireContext(), label, value, "已复制" + label);
    }

    private void link() {
        Loaded before = loaded;
        List<String> linked = ListPolicy.copyOrEmpty(
            before == null || before.profile() == null ? null : before.profile().providers());
        LoginSheet.show(requireActivity(), "link", failure -> {
            if (!isAdded()) return;
            if (!failure.isEmpty()) {
                if (!LoginSheet.CANCELLED.equals(failure)) MsToast.show(requireContext(), failure);
                return;
            }
            reload(result -> {
                if (result.profile() == null) return;
                for (String provider : PROVIDERS) {
                    if (result.profile().linked(provider) && !linked.contains(provider)) {
                        MsToast.show(requireContext(), "已关联 " + result.profile().providerName(provider));
                        return;
                    }
                }
            });
        });
    }

    private void signIn() {
        SignIn.start(requireActivity(), failure -> {
            if (!isAdded()) return;
            if (failure.isEmpty()) reload(null);
            else if (!LoginSheet.CANCELLED.equals(failure)) MsToast.show(requireContext(), failure);
        });
    }

    private void chooseAvatar() {
        if (busy) return;
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        avatarPickerSessionId = state.sessionId();
        picker.launch("image/*");
    }

    private void onPicked(@Nullable Uri uri) {
        String expectedSessionId = avatarPickerSessionId;
        avatarPickerSessionId = null;
        if (uri == null || getView() == null) return;
        if (expectedSessionId == null || expectedSessionId.isEmpty()) return;
        run(context -> {
            byte[] image;
            try (InputStream input = context.getContentResolver().openInputStream(uri)) {
                if (input == null) return "读不到这张图片";
                image = HttpBodyPolicy.readBounded(input, DeviceDataApi.MAX_AVATAR_BYTES);
            } catch (IOException | SecurityException unreadable) {
                return "读不到这张图片";
            }
            if (image == null) return "图片超过 1 MB，请换一张小一些的";
            if (DeviceDataApi.avatarType(image) == null) return "头像只支持 PNG 或 JPEG 图片";
            BitmapFactory.Options size = BitmapPolicy.decodeBounds(image);
            if (size == null) return "读不到这张图片";
            if (BitmapPolicy.longestEdge(size.outWidth, size.outHeight) > MAX_AVATAR_UPLOAD_EDGE) return "图片尺寸太大，请换一张小一些的";
            // Rust 从文件读头像：写进应用私有的缓存目录，上传完就删。
            File file;
            try {
                file = File.createTempFile("avatar-", ".img", context.getCacheDir());
            } catch (IOException unwritable) {
                return "存储空间不可用，请稍后再试";
            }
            try {
                try (OutputStream output = Files.newOutputStream(file.toPath(), StandardOpenOption.WRITE,
                        StandardOpenOption.TRUNCATE_EXISTING, LinkOption.NOFOLLOW_LINKS)) {
                    output.write(image);
                } catch (IOException unwritable) {
                    return "存储空间不可用，请稍后再试";
                }
                new DeviceDataApi(context, expectedSessionId).uploadAvatar(file);
            } finally {
                FilePolicy.deleteQuietly(file);
            }
            return "";
        }, "头像已更新");
    }

    private void export() {
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        String expectedSessionId = state.sessionId();
        long request = generation;
        String name = DeviceDataApi.exportCacheFileName(LocalDate.now(ZoneId.systemDefault()), UUID.randomUUID());
        execute(context -> {
            File directory = new File(context.getCacheDir(), EXPORTS);
            if (!directory.isDirectory() && !directory.mkdirs()) return "没有导出，存储空间不可用";
            // 已分享的 URI 必须保持指向原来的归档；只清理超过一天的旧导出。
            File[] previous = directory.listFiles();
            long cutoff = System.currentTimeMillis() - EXPORT_RETENTION_MILLIS;
            if (previous != null) for (File stale : previous) {
                if (stale.getName().startsWith("msime-data-")
                        && stale.getName().endsWith(".zip")
                        && Files.isRegularFile(stale.toPath(), LinkOption.NOFOLLOW_LINKS)
                        && stale.lastModified() < cutoff) FilePolicy.deleteQuietly(stale);
            }
            File file = new File(directory, name);
            // Rust 先写进同目录的临时文件，读得出 zip 的中央目录才换到 file；失败时 file 不会出现。
            try {
                new DeviceDataApi(context, expectedSessionId).exportData(file);
            } catch (AccountStore.Failure failure) {
                FilePolicy.deleteQuietly(file);
                if ("account_rate_limited".equals(failure.code)) return "今天的导出次数已用完，明天再试";
                if ("account_storage".equals(failure.code)) return "没有导出，存储空间不可用";
                throw failure;
            }
            return "";
        }, outcome -> {
            if (!outcome.isEmpty()) {
                MsToast.show(requireContext(), outcome);
                return;
            }
            Context context = requireContext();
            File file = new File(new File(context.getCacheDir(), EXPORTS), name);
            if (request != generation || !sameLoadedSession(expectedSessionId)) {
                FilePolicy.deleteQuietly(file);
                MsToast.show(context, "登录已改变，请重新打开页面");
                return;
            }
            Uri uri = FileProvider.getUriForFile(context, context.getPackageName() + ".files", file);
            Intent send = new Intent(Intent.ACTION_SEND).setType("application/zip").putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(Intent.createChooser(send, "导出我的数据"));
        }, null);
    }

    private void confirmDeleteData() {
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        String expectedSessionId = state.sessionId();
        String size = state == null || state.data() == null ? "" : "（约 " + DeviceDataApi.formatBytes(state.data().bytes()) + "）";
        new OptionSheet(requireContext(), "删除云端数据" + size,
            "删除云端的设置、词库、常用语、云剪贴板和这台设备上传过的语音样本，账号和社区作品保留，本机数据不受影响")
            .destructive("删除云端数据", () -> recentLogin(sessionId -> context -> {
                DeviceDataApi.VoiceSamples voice =
                    new DeviceDataApi(context, sessionId).deleteData(DeviceDataApi.DELETABLE_SECTIONS);
                return voice.ok() ? "" : NOTICE + "云端数据已删除。"
                    + AccountFailureText.voiceSamplesLeft(voice.code(), voice.retryAfterSeconds());
            }, expectedSessionId, "云端数据已删除", false))
            .show();
    }

    private void confirmSignOut() {
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        String expectedSessionId = state.sessionId();
        new OptionSheet(requireContext(), "退出登录", "本机的词库和设置会保留，云同步随之关闭")
            .destructive("退出登录", () -> run(context -> {
                SignIn.Logout result = SignIn.logout(context, expectedSessionId);
                if (!result.signedOut()) return result.message();
                return result.message().isEmpty() ? "" : NOTICE + result.message();
            }, "已退出登录", true))
            .show();
    }

    private void confirmDeleteAccount() {
        Loaded state = loaded;
        if (state == null || state.profile() == null || state.sessionId().isEmpty()) return;
        String expectedSessionId = state.sessionId();
        new OptionSheet(requireContext(), "注销账号", "云端的词库、皮肤、设置和社区作品会立即永久删除")
            .destructive("继续注销", () -> new OptionSheet(requireContext(), "确定注销？", "这一步无法撤销")
                .destructive("永久注销账号", () -> recentLogin(sessionId -> context -> {
                    DeviceDataApi.VoiceSamples voice = new DeviceDataApi(context, sessionId).deleteAccount();
                    // 注销成功时 Rust 已经清掉本地会话，这里只清本机的同步状态。
                    SignIn.signOut(context);
                    return voice.ok() ? "" : NOTICE + "账号已注销。"
                        + AccountFailureText.voiceSamplesLeft(voice.code(), voice.retryAfterSeconds());
                }, expectedSessionId, "账号已注销", true))
                .show())
            .show();
    }

    // ---- 执行 ----

    /** 一次要联网的动作：返回空字符串表示成功，以 {@link #NOTICE} 开头表示成功但另有一句要说（例如语音样本没删掉），否则是给人看的失败说明。 */
    private interface Work {
        String run(Context context) throws AccountStore.Failure, DeviceDataApi.RecentLoginRequired;
    }

    /** 内部记号：成功了，但后面那句话要代替成功提示显示出来，不会显示给用户。 */
    private static final String NOTICE = "\u0001";

    private void done(String outcome, @Nullable String success, boolean leave) {
        if (outcome.isEmpty()) finish(success, leave);
        else if (outcome.startsWith(NOTICE)) finish(outcome.substring(NOTICE.length()), leave);
        else MsToast.show(requireContext(), outcome);
    }

    private void run(Work work, @Nullable String success) {
        run(work, success, false);
    }

    private void run(Work work, @Nullable String success, boolean leave) {
        execute(work, outcome -> done(outcome, success, leave), null);
    }

    /**
     * 要求最近登录的动作：服务端说要重新登录时弹登录面板，重新登录的还是同一个账号才重试一次。
     */
    private void recentLogin(Function<String, Work> workFactory, String expectedSessionId,
            String success, boolean leave) {
        Loaded initial = loaded;
        if (initial == null || initial.profile() == null || !expectedSessionId.equals(initial.sessionId())) return;
        String before = initial.profile().id();
        execute(workFactory.apply(expectedSessionId), outcome -> done(outcome, success, leave), () -> {
            MsToast.show(requireContext(), "为了安全，请重新登录一次");
            SignIn.start(requireActivity(), failure -> {
                if (!isAdded()) return;
                if (!failure.isEmpty()) {
                    if (!LoginSheet.CANCELLED.equals(failure)) MsToast.show(requireContext(), failure);
                    return;
                }
                reload(result -> {
                    if (result.profile() == null || result.sessionId().isEmpty()
                            || before.isEmpty() || !before.equals(result.profile().id())) {
                        MsToast.show(requireContext(), "登录的不是原来的账号，操作已取消");
                        return;
                    }
                    run(workFactory.apply(result.sessionId()), success, leave);
                });
            });
        });
    }

    private boolean sameLoadedSession(String expectedSessionId) {
        Loaded state = loaded;
        return state != null && expectedSessionId.equals(state.sessionId());
    }

    private void execute(Work work, Consumer<String> done, @Nullable Runnable relogin) {
        if (busy) return;
        busy = true;
        Function<Context, String> task = context -> {
            try {
                return work.run(context);
            } catch (DeviceDataApi.RecentLoginRequired required) {
                return relogin == null ? "需要重新登录后再试" : RELOGIN;
            } catch (AccountStore.Failure failure) {
                return AccountFailureText.profile(failure.code, failure.retryAfterSeconds);
            }
        };
        HostTask.runNetwork(this, task, outcome -> {
            busy = false;
            if (outcome == null) {
                MsToast.show(requireContext(), "没有完成，请稍后再试");
            } else if (RELOGIN.equals(outcome) && relogin != null) {
                relogin.run();
            } else {
                done.accept(outcome);
            }
        });
    }

    /** 内部记号：工作线程告诉主线程「要重新登录」，不会显示给用户。 */
    private static final String RELOGIN = "\u0000relogin";

    private void finish(@Nullable String success, boolean leave) {
        if (success != null) MsToast.show(requireContext(), success);
        if (leave) {
            requireActivity().getOnBackPressedDispatcher().onBackPressed();
        } else {
            reload(null);
        }
    }

}
