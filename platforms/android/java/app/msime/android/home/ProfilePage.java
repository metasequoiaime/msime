package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
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
import app.msime.android.BitmapPolicy;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.time.LocalDate;
import java.time.ZoneId;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Function;
import javax.net.ssl.HttpsURLConnection;

/**
 * 个人资料：头像（点了换一张）、昵称、水杉 ID、邮箱，三种登录方式的关联状态，云端数据（大小、导出、删除），退出登录与注销账号。
 *
 * <p>数据全部来自 {@link DeviceDataApi}。注销账号和删除云端数据要求最近登录：收到 {@link DeviceDataApi.RecentLoginRequired} 时弹登录面板，用户重新登录同一个账号后自动重试一次；登录成了别的账号时不重试，免得删错。关联登录方式走登录面板的 `link` 用途，回来后按关联列表的变化说出关联了哪一种。
 */
public final class ProfilePage extends DetailPage {
    private static final String[][] PROVIDERS = {{"apple", "Apple"}, {"google", "Google"}, {"email", "邮箱"}};
    private static final String EXPORTS = "exports";
    /** 头像解码后的长边上限（像素）：最大的头像画 88dp，xxxhdpi 下约 352 像素，留一点余量。 */
    private static final int AVATAR_DECODE_EDGE = 384;
    /** 允许上传的头像长边上限（像素）：1 MiB 的平涂 PNG 可以有上万像素宽，解码会吃掉上 GB 内存。 */
    private static final int MAX_AVATAR_UPLOAD_EDGE = 8192;

    private final ActivityResultLauncher<String> picker =
        registerForActivityResult(new ActivityResultContracts.GetContent(), this::onPicked);
    @Nullable private LinearLayout column;
    @Nullable private Loaded loaded;
    private boolean busy;

    /** 一次读取的结果：资料（未登录或读不到时为 null）、读不到的原因、云端数据汇总（读不到时为 null）、后端开放的登录方式、头像。 */
    private record Loaded(@Nullable DeviceDataApi.Profile profile, @Nullable CloudApi.Failure failure,
            @Nullable DeviceDataApi.DataSummary data, CloudApi.Providers providers, @Nullable Bitmap avatar) {}

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        render();
    }

    @Override protected void onBecameVisible() { reload(null); }

    private void reload(@Nullable Consumer<Loaded> then) {
        HostTask.runNetwork(this, ProfilePage::load, result -> {
            if (result == null) return;
            loaded = result;
            render();
            if (then != null) then.accept(result);
        });
    }

    private static Loaded load(Context context) {
        CloudApi.Providers providers = SignIn.providers(context);
        DeviceDataApi api = new DeviceDataApi(context);
        DeviceDataApi.Profile profile;
        try {
            profile = api.profile();
        } catch (CloudApi.Failure failure) {
            return new Loaded(null, failure, null, providers, null);
        }
        bindIfNeeded(context, profile);
        DeviceDataApi.DataSummary data;
        try {
            data = api.dataSummary();
        } catch (CloudApi.Failure unavailable) {
            data = null;
        }
        return new Loaded(profile, null, data, providers, avatar(profile.avatarUrl()));
    }

    /** 同步状态记着的账号与服务端说的不一致（登录时没读到用户 id，或换了账号）时重新绑定；绑定换账号时会关闭同步并清空游标。 */
    static void bindIfNeeded(Context context, DeviceDataApi.Profile profile) {
        String kind = SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)) ? SyncSwitch.loginKind(context)
            : profile.loginKind();
        if (kind.isEmpty()) return;
        if (profile.id().equals(SyncSwitch.accountId(context)) && kind.equals(SyncSwitch.loginKind(context))) return;
        SyncSwitch.bindAccount(context, profile.id(), kind);
    }

    /** 读头像图片：只认 https、不超过 1 MiB；读不到时为 null，界面显示首字头像。阻塞。 */
    @Nullable static Bitmap avatar(String url) {
        if (url == null || !url.startsWith("https://")) return null;
        try {
            HttpsURLConnection connection = (HttpsURLConnection) new URL(url).openConnection();
            try {
                connection.setInstanceFollowRedirects(false);
                connection.setConnectTimeout(10_000);
                connection.setReadTimeout(15_000);
                connection.setRequestProperty("User-Agent", CloudApi.USER_AGENT);
                if (connection.getResponseCode() / 100 != 2) return null;
                try (InputStream input = connection.getInputStream()) {
                    byte[] bytes = HttpBodyPolicy.readBounded(input, DeviceDataApi.MAX_AVATAR_BYTES);
                    return bytes == null ? null : decodeAvatar(bytes);
                }
            } finally {
                connection.disconnect();
            }
        } catch (IOException | RuntimeException unavailable) {
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
        GradientDrawable circle = Ui.circle(Ui.accent(context));
        if (image != null) {
            ImageView picture = new ImageView(context);
            picture.setImageBitmap(image);
            picture.setScaleType(ImageView.ScaleType.CENTER_CROP);
            ViewPolicy.setBackground(picture, circle);
            picture.setClipToOutline(true);
            Ui.hideFromAccessibility(picture);
            frame.addView(picture, Ui.squareFrameParams(context, sizeDp));
        } else {
            TextView letter = Ui.styledLabel(context, Ui.trimmedInitial(name, "?"), Math.round(sizeDp * 0.4f), 600,
                Ui.onAccent(context));
            ViewPolicy.setCentered(letter);
            ViewPolicy.setBackground(letter, circle);
            Ui.hideFromAccessibility(letter);
            frame.addView(letter, Ui.squareFrameParams(context, sizeDp));
        }
        return frame;
    }

    private void render() {
        LinearLayout column = this.column;
        if (column == null) return;
        column.removeAllViews();
        Loaded state = loaded;
        if (state == null) {
            GroupCard.add(column, null).note("正在读取…");
            return;
        }
        if (state.profile() == null) {
            renderUnavailable(column, state.failure());
            return;
        }
        DeviceDataApi.Profile profile = state.profile();
        Context context = requireContext();
        column.addView(header(context, profile, state.avatar()));

        GroupCard account = GroupCard.add(column, "账号").withDividers(16);
        account.nav("昵称", null, profile.displayName(), this::rename);
        account.nav("水杉 ID", null, profile.id(), () -> copy("水杉 ID", profile.id()));
        if (!profile.email().isEmpty()) account.value("邮箱", null, profile.email());

        GroupCard methods = GroupCard.add(column, "登录方式").withDividers(16);
        for (String[] provider : PROVIDERS) {
            boolean linked = profile.linked(provider[0]);
            if (!linked && !offered(state.providers(), provider[0])) continue;
            methods.nav(provider[1], null, linked ? "已关联" : "关联", linked ? null : this::link);
        }
        methods.footer("关联后可以用任意一种方式登录同一个账号");

        GroupCard data = GroupCard.add(column, "数据").withDividers(16);
        data.nav("云端数据", null, state.data() == null ? null : DeviceDataApi.formatBytes(state.data().bytes()),
            this::confirmDeleteData);
        data.nav("导出我的数据", null, null, this::export);

        GroupCard signOut = GroupCard.add(column, null);
        signOut.addView(dangerButton(context, "退出登录", this::confirmSignOut));
        GroupCard delete = GroupCard.add(column, null);
        delete.addView(dangerButton(context, "注销账号", this::confirmDeleteAccount));
        delete.footer("注销后云端的词库、皮肤、设置和社区作品会立即永久删除，无法恢复");
    }

    private void renderUnavailable(LinearLayout column, @Nullable CloudApi.Failure failure) {
        GroupCard card = GroupCard.add(column, null);
        if (failure != null && failure.signedOut()) {
            card.note("登录后可以在这里修改昵称和头像、关联登录方式、导出或删除云端数据。");
            card.button("登录", "Google、Apple 或邮箱", "登录", this::signIn);
        } else {
            card.note(failure != null && failure.network() ? "连不上服务器，请检查网络后再试。"
                : "个人资料暂时读不到，请稍后再试。");
            card.button("重新读取", null, "重试", () -> reload(null));
        }
    }

    private View header(Context context, DeviceDataApi.Profile profile, @Nullable Bitmap image) {
        LinearLayout header = Ui.column(context);
        ViewPolicy.setCenteredHorizontally(header);
        Ui.setPaddingDp(header, context, 0, 8, 0, 4);

        FrameLayout avatar = new FrameLayout(context);
        avatar.addView(avatarView(context, 88, profile.displayName(), image));
        ImageView camera = Ui.decorativeIcon(context, app.msime.android.R.drawable.ms_w5_me_camera,
            Ui.text(context));
        GradientDrawable badge = Ui.circle(Ui.card(context));
        ViewPolicy.setBackground(camera, badge);
        int pad = Ui.dp(context, 6);
        Ui.setSymmetricPaddingPx(camera, pad);
        Ui.hideFromAccessibility(camera);
        FrameLayout.LayoutParams cameraParams = Ui.squareFrameParams(context, 28);
        cameraParams.gravity = Gravity.BOTTOM | Gravity.END;
        avatar.addView(camera, cameraParams);
        ViewPolicy.setInteractive(avatar, true);
        avatar.setContentDescription("更换头像");
        ViewPolicy.bindClick(avatar, this::chooseAvatar);
        header.addView(avatar, Ui.squareParams(context, 92));

        TextView name = Ui.styledLabel(context, profile.displayName(), 22, 700, Ui.text(context));
        LinearLayout.LayoutParams nameParams = Ui.wrap();
        nameParams.topMargin = Ui.dp(context, 10);
        header.addView(name, nameParams);

        if (!profile.email().isEmpty()) {
            TextView email = Ui.styledLabel(context, profile.email(), Ui.TEXT_ROW_SUBTITLE, 400,
                Ui.subText(context));
            LinearLayout.LayoutParams emailParams = Ui.wrap();
            emailParams.topMargin = Ui.dp(context, 2);
            header.addView(email, emailParams);
        }

        String kind = SyncSwitch.validLoginKind(SyncSwitch.loginKind(context)) ? SyncSwitch.loginKind(context)
            : profile.loginKind();
        if (!kind.isEmpty()) {
            TextView chip = Ui.styledLabel(context, "通过 " + providerName(kind) + " 登录", 12, 500,
                Ui.accent(context));
            ViewPolicy.setBackground(chip, Ui.pill(Ui.accentSoft(context)));
            Ui.setSymmetricPaddingDp(chip, context, 10, 3);
            LinearLayout.LayoutParams chipParams = Ui.wrap();
            chipParams.topMargin = Ui.dp(context, 8);
            header.addView(chip, chipParams);
        }
        return header;
    }

    private static View dangerButton(Context context, CharSequence label, Runnable action) {
        TextView button = Ui.textButton(context, label, Ui.TEXT_ROW_TITLE, 500, Ui.danger(context),
            Ui.ripple(context), Ui.ACTION_BUTTON_MIN_HEIGHT, action);
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

    static String providerName(String kind) {
        for (String[] provider : PROVIDERS) if (provider[0].equals(kind)) return provider[1];
        return kind;
    }

    // ---- 动作 ----

    private void rename() {
        Loaded state = loaded;
        if (state == null || state.profile() == null) return;
        InputDialog dialog = new InputDialog(requireContext(), "昵称", "最多 64 个字，留空恢复默认昵称");
        dialog.addField("昵称", state.profile().displayName(), 0)
            .setFilters(new InputFilter[] {new InputFilter.LengthFilter(DeviceDataApi.MAX_DISPLAY_NAME * 2)});
        dialog.setValidator(values -> DeviceDataApi.validDisplayName(values.get(0)));
        dialog.setPrimary("保存", values -> run(context -> {
            new DeviceDataApi(context).rename(values.get(0));
            return "";
        }, "昵称已更新"));
        dialog.show();
    }

    private void copy(String label, String value) {
        ClipboardActions.copyText(requireContext(), label, value, "已复制" + label);
    }

    private void link() {
        Loaded before = loaded;
        List<String> linked = before == null || before.profile() == null ? List.of() : before.profile().providers();
        LoginSheet.show(requireActivity(), "link", failure -> {
            if (!isAdded()) return;
            if (!failure.isEmpty()) {
                if (!LoginSheet.CANCELLED.equals(failure)) MsToast.show(requireContext(), failure);
                return;
            }
            reload(result -> {
                if (result.profile() == null) return;
                for (String[] provider : PROVIDERS) {
                    if (result.profile().linked(provider[0]) && !linked.contains(provider[0])) {
                        MsToast.show(requireContext(), "已关联 " + provider[1]);
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
        picker.launch("image/*");
    }

    private void onPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
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
            new DeviceDataApi(context).uploadAvatar(image);
            return "";
        }, "头像已更新");
    }

    private void export() {
        String name = DeviceDataApi.exportFileName(LocalDate.now(ZoneId.systemDefault()));
        execute(context -> {
            File directory = new File(context.getCacheDir(), EXPORTS);
            if (!directory.isDirectory() && !directory.mkdirs()) return "没有导出，存储空间不可用";
            // 导出包里是个人数据，只留最新这一份给分享用：先删掉以前导出的，不让它们一直堆在缓存目录里。
            File[] previous = directory.listFiles();
            if (previous != null) for (File stale : previous) deleteQuietly(stale);
            File file = new File(directory, name);
            try (OutputStream output = new FileOutputStream(file)) {
                new DeviceDataApi(context).exportData(output);
            } catch (IOException unwritable) {
                deleteQuietly(file);
                return "没有导出，存储空间不可用";
            } catch (CloudApi.Failure failure) {
                deleteQuietly(file);
                if (failure.status == 429) return "今天的导出次数已用完，明天再试";
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
            Uri uri = FileProvider.getUriForFile(context, context.getPackageName() + ".files", file);
            Intent send = new Intent(Intent.ACTION_SEND).setType("application/zip").putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(Intent.createChooser(send, "导出我的数据"));
        }, null);
    }

    private static void deleteQuietly(File file) {
        if (file.exists() && !file.delete()) file.deleteOnExit();
    }

    private void confirmDeleteData() {
        Loaded state = loaded;
        String size = state == null || state.data() == null ? "" : "（约 " + DeviceDataApi.formatBytes(state.data().bytes()) + "）";
        new OptionSheet(requireContext(), "删除云端数据" + size,
            "删除云端的设置、词库、常用语和云剪贴板，账号和社区作品保留，本机数据不受影响")
            .destructive("删除云端数据", () -> recentLogin(context -> {
                new DeviceDataApi(context).deleteData(DeviceDataApi.DELETABLE_SECTIONS);
                return "";
            }, "云端数据已删除", false))
            .show();
    }

    private void confirmSignOut() {
        new OptionSheet(requireContext(), "退出登录", "本机的词库和设置会保留，云同步随之关闭")
            .destructive("退出登录", () -> run(context -> {
                SignIn.signOut(context);
                return "";
            }, "已退出登录", true))
            .show();
    }

    private void confirmDeleteAccount() {
        new OptionSheet(requireContext(), "注销账号", "云端的词库、皮肤、设置和社区作品会立即永久删除")
            .destructive("继续注销", () -> new OptionSheet(requireContext(), "确定注销？", "这一步无法撤销")
                .destructive("永久注销账号", () -> recentLogin(context -> {
                    new DeviceDataApi(context).deleteAccount();
                    SignIn.signOut(context);
                    return "";
                }, "账号已注销", true))
                .show())
            .show();
    }

    // ---- 执行 ----

    /** 一次要联网的动作：返回空字符串表示成功，否则是给人看的失败说明。 */
    private interface Work {
        String run(Context context) throws CloudApi.Failure, DeviceDataApi.RecentLoginRequired;
    }

    private void run(Work work, @Nullable String success) {
        run(work, success, false);
    }

    private void run(Work work, @Nullable String success, boolean leave) {
        execute(work, outcome -> {
            if (outcome.isEmpty()) finish(success, leave);
            else MsToast.show(requireContext(), outcome);
        }, null);
    }

    /**
     * 要求最近登录的动作：服务端说要重新登录时弹登录面板，重新登录的还是同一个账号才重试一次。
     */
    private void recentLogin(Work work, String success, boolean leave) {
        execute(work, outcome -> {
            if (outcome.isEmpty()) finish(success, leave);
            else MsToast.show(requireContext(), outcome);
        }, () -> {
            String before = loaded == null || loaded.profile() == null ? "" : loaded.profile().id();
            MsToast.show(requireContext(), "为了安全，请重新登录一次");
            SignIn.start(requireActivity(), failure -> {
                if (!isAdded()) return;
                if (!failure.isEmpty()) {
                    if (!LoginSheet.CANCELLED.equals(failure)) MsToast.show(requireContext(), failure);
                    return;
                }
                reload(result -> {
                    if (result.profile() == null || before.isEmpty() || !before.equals(result.profile().id())) {
                        MsToast.show(requireContext(), "登录的不是原来的账号，操作已取消");
                        return;
                    }
                    run(work, success, leave);
                });
            });
        });
    }

    private void execute(Work work, Consumer<String> done, @Nullable Runnable relogin) {
        if (busy) return;
        busy = true;
        Function<Context, String> task = context -> {
            try {
                return work.run(context);
            } catch (DeviceDataApi.RecentLoginRequired required) {
                return relogin == null ? "需要重新登录后再试" : RELOGIN;
            } catch (CloudApi.Failure failure) {
                return explain(failure);
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

    static String explain(CloudApi.Failure failure) {
        if (failure.network()) return "连不上服务器，请检查网络后再试";
        if (failure.signedOut()) return "登录已失效，请重新登录";
        if (failure.status == 429) return "操作太频繁，请稍后再试";
        if (failure.status == 413) return "图片超过 1 MB，请换一张小一些的";
        if (failure.status == 415 || "invalid_avatar".equals(failure.code)) return "头像只支持 PNG 或 JPEG 图片";
        if (failure.unavailable() || failure.status == 503) return "这项功能暂未开放";
        return "没有完成，请稍后再试";
    }

}
