package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import android.content.ContentResolver;
import android.content.Context;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.DrawablePolicy;
import app.msime.android.AppVersionPolicy;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountStore;
import app.msime.android.BitmapPolicy;
import app.msime.android.FilePolicy;
import app.msime.android.FeedbackApi;
import app.msime.android.FeedbackImagePolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Future;
import org.json.JSONObject;

/**
 * 帮助与反馈：反馈类型（问题 / 建议 / 词库纠错）、附带诊断信息开关、描述（0 / 500）、至多 3 张截图，提交到水杉云（{@link FeedbackApi}）。
 *
 * <p>附带诊断信息默认关闭；打开后只带服务端白名单里的字段——机型、系统版本、应用版本与版本（edition）、当前方案、键盘布局、皮肤、键盘是否已启用和是否为默认——不带任何输入内容和日志。截图由用户从系统选择器里挑，这里按长边 1600 像素、1 MiB 以内重新编码成 JPEG，顺带去掉 EXIF（其中可能有位置）；提交时写进缓存目录交给 Rust 读，提交完就删。
 *
 * <p>没有登录也能提交：请求用设备的匿名账号。另外保留 QQ 群、Telegram 群组和 GitHub Issues 三个渠道，QQ 群号与 Telegram 链接和其他平台一字不差（`scripts/test-support-channels.py` 守着）。
 */
public final class FeedbackPage extends DetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("反馈", "其他渠道");
    }

    private static final String QQ_GROUP = "829919142";
    private static final String TELEGRAM_URL = "https://t.me/msimegroup";
    private static final String ISSUES = "https://github.com/metasequoiaime/msime/issues/new";
    private static final int MAX_EDGE = 1600;

    private FeedbackApi.Type type = FeedbackApi.Type.BUG;
    private boolean diagnostics;
    private String draft = "";
    private final List<byte[]> screenshots = new ArrayList<>(FeedbackApi.MAX_SCREENSHOTS);
    private boolean sending;
    private boolean sent;
    @Nullable private Future<?> submitTask;

    @Nullable private EditText detail;
    @Nullable private TextView counter;
    @Nullable private TextView submit;
    @Nullable private LinearLayout thumbnails;
    @Nullable private View addShot;

    private final ActivityResultLauncher<String> picker =
        registerForActivityResult(new ActivityResultContracts.GetMultipleContents(), this::onPicked);

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();

        // 类型和诊断开关是这张表单的一部分，和描述、提交放在同一个分类里，不用切分类再回来提交。
        GroupCard kind = GroupCard.add(sectionColumn(column, "反馈"), "类型");
        GroupCard.Row[] typeRow = new GroupCard.Row[1];
        typeRow[0] = kind.nav("反馈类型", null, type.title(), () -> chooseType(typeRow[0]));
        kind.toggle("附带诊断信息", "包含机型、系统版本、当前方案与键盘设置，不含输入内容", diagnostics,
            checked -> diagnostics = checked);

        GroupCard description = GroupCard.add(sectionColumn(column, "反馈"), "描述");
        LinearLayout card = description.card();
        EditText input = ViewPolicy.styledInput(context, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        input.setHint("遇到了什么问题？可以写复现步骤、出错的词或期望的结果");
        input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE
            | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        ViewPolicy.setTopStart(input);
        ViewPolicy.setMinLines(input, 4);
        ViewPolicy.clearBackground(input);
        input.setHintTextColor(ThemeColorPolicy.subText(context));
        ViewPolicy.setSymmetricPaddingDp(requireContext(), input, 16, 14);
        input.setText(draft);
        input.setContentDescription("描述");
        card.addView(input, LayoutPolicy.matchWidthWrapParams());
        detail = input;

        View rule = ViewPolicy.newColorView(context, ThemeColorPolicy.hairline(context));
        LinearLayout.LayoutParams ruleParams = LayoutPolicy.matchWidthHeightPx(
            DimensionPolicy.atLeastOnePixel(context, 0.5f));
        ruleParams.setMarginStart(DimensionPolicy.pixels(requireContext(), 16));
        card.addView(rule, ruleParams);

        HorizontalScrollView strip = new HorizontalScrollView(context);
        strip.setHorizontalScrollBarEnabled(false);
        LinearLayout shots = LayoutPolicy.row(context);
        ViewPolicy.setPaddingDp(requireContext(), shots, 16, 10, 16, 0);
        strip.addView(shots);
        card.addView(strip, LayoutPolicy.matchWidthWrapParams());
        thumbnails = shots;

        LinearLayout add = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(add);
        ViewPolicy.setPaddingDp(requireContext(), add, 16, 12, 16, 14);
        add.setContentDescription("添加截图，最多 " + FeedbackApi.MAX_SCREENSHOTS + " 张");
        ImageView icon = ImageViewPolicy.decorative(context, R.drawable.ms_w4_me2_image, ThemeColorPolicy.accent(context));
        add.addView(icon, LayoutPolicy.squareParams(requireContext(), 20));
        TextView label = ViewPolicy.styledLabel(context, "添加截图", Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.accent(context));
        LinearLayout.LayoutParams labelParams = LayoutPolicy.wrapParams();
        labelParams.setMarginStart(DimensionPolicy.pixels(requireContext(), 10));
        add.addView(label, labelParams);
        ViewPolicy.makeClickable(add, context, () -> picker.launch("image/*"));
        card.addView(add, LayoutPolicy.matchWidthWrapParams());
        addShot = add;

        TextView count = ViewPolicy.styledLabel(context, "", 13, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setPaddingDp(requireContext(), count, Ui.GROUP_TITLE_INSET, 6,
            Ui.GROUP_TITLE_INSET, 0);
        description.view().addView(count, LayoutPolicy.matchWidthWrapParams());
        counter = count;

        TextView button = ViewPolicy.textButton(context, "", 16, 600, ThemeColorPolicy.onAccent(context), null,
            Ui.ACTION_BUTTON_MIN_HEIGHT, this::submit);
        ViewPolicy.setPoliteLiveRegion(button);
        LinearLayout.LayoutParams buttonParams = LayoutPolicy.matchWidthWrapParams();
        buttonParams.topMargin = DimensionPolicy.pixels(requireContext(), Ui.GROUP_GAP);
        sectionColumn(column, "反馈").addView(button, buttonParams);
        submit = button;

        ViewPolicy.afterTextChanged(input, text -> {
            draft = text.toString();
            if (sent) sent = false;
            refresh();
        });

        GroupCard channels = GroupCard.add(sectionColumn(column, "其他渠道"), "其他渠道").withDividers(Ui.ROW_PADDING_H);
        channels.button("QQ 群", QQ_GROUP, "复制群号", () -> copy("QQ 群号", QQ_GROUP, "已复制群号"));
        channels.button("Telegram 群组", "t.me/msimegroup", "打开", this::openTelegram);
        channels.button("GitHub Issues", "公开的问题单，适合附上复现步骤", "打开",
            () -> AboutPage.openLink(context, ISSUES));
        channels.button("复制设备信息", "品牌、型号、系统与应用版本、存储空间等，贴进反馈里方便排查", "复制",
            () -> DeviceInfo.copy(this));

        renderThumbnails();
        refresh();
    }

    @Override public void onDestroyView() {
        if (submitTask != null) {
            submitTask.cancel(true);
            submitTask = null;
        }
        // The submission result is fenced to this view; reset its transient state so a rebuilt
        // page does not keep the submit button disabled after the old callback is dropped.
        sending = false;
        detail = null;
        counter = null;
        submit = null;
        thumbnails = null;
        addShot = null;
        super.onDestroyView();
    }

    private void chooseType(GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "反馈类型", null);
        for (FeedbackApi.Type value : FeedbackApi.Type.values()) {
            sheet.option(value.title(), value == type, () -> {
                type = value;
                row.setValue(value.title());
            });
        }
        sheet.show();
    }

    /** 计数、提交按钮的样子与可点状态。 */
    private void refresh() {
        Context context = getContext();
        if (context == null || counter == null || submit == null) return;
        int length = TextPolicy.codePointLength(draft);
        counter.setText(length + " / " + FeedbackApi.MAX_TEXT);
        ViewPolicy.setTextColor(counter,
            length > FeedbackApi.MAX_TEXT ? ThemeColorPolicy.danger(context) : ThemeColorPolicy.subText(context));
        boolean ready = !sending && !sent && FeedbackApi.validText(draft);
        submit.setText(sent ? "已提交" : sending ? "正在提交…" : "提交");
        ViewPolicy.setEnabled(submit, ready);
        ViewPolicy.setTextColor(submit, ready ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.subText(context));
        int fill = ready ? ThemeColorPolicy.accent(context)
            : ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorSurfaceContainerHighest);
        ViewPolicy.setBackground(submit, ViewPolicy.ripple(context, fill, DimensionPolicy.pixels(requireContext(), Ui.GROUP_RADIUS)));
        if (addShot != null) ViewPolicy.setEnabledWithAlpha(addShot,
            screenshots.size() < FeedbackApi.MAX_SCREENSHOTS && !sending, 0.38f);
    }

    private void renderThumbnails() {
        LinearLayout strip = thumbnails;
        if (strip == null) return;
        Context context = strip.getContext();
        strip.removeAllViews();
        ViewPolicy.setVisible((View) strip.getParent(), !screenshots.isEmpty());
        for (int index = 0; index < screenshots.size(); index++) {
            byte[] bytes = screenshots.get(index);
            int position = index;
            FrameLayout frame = new FrameLayout(context);
            ImageView image = new ImageView(context);
            image.setScaleType(ImageView.ScaleType.CENTER_CROP);
            BitmapFactory.Options options = new BitmapFactory.Options();
            options.inSampleSize = 4;
            image.setImageBitmap(BitmapFactory.decodeByteArray(bytes, 0, bytes.length, options));
            ViewPolicy.setBackground(image, DrawablePolicy.rounded(ThemeColorPolicy.rowBackground(context), DimensionPolicy.pixels(requireContext(), 10)));
            image.setClipToOutline(true);
            image.setContentDescription("截图 " + (index + 1));
            frame.addView(image, LayoutPolicy.squareFrameParams(requireContext(), Ui.THUMBNAIL_SIZE));
            ImageView remove = new ImageView(context);
            remove.setImageResource(R.drawable.ms_w4_me2_close);
            ImageViewPolicy.setTint(remove,
                ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorOnSurfaceInverse));
            ViewPolicy.setBackground(remove, DrawablePolicy.pill(ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorSurfaceInverse)));
            ViewPolicy.setSymmetricPaddingDp(requireContext(), remove, 3, 3);
            remove.setContentDescription("移除截图 " + (index + 1));
            ViewPolicy.bindClick(remove, () -> {
                if (sending) return;
                screenshots.remove(position);
                renderThumbnails();
                refresh();
            });
            FrameLayout.LayoutParams removeParams = LayoutPolicy.squareFrameParams(requireContext(), 20);
            removeParams.gravity = Gravity.TOP | Gravity.END;
            frame.addView(remove, removeParams);
            LinearLayout.LayoutParams params = LayoutPolicy.squareParams(requireContext(), Ui.THUMBNAIL_SIZE);
            params.setMarginEnd(DimensionPolicy.pixels(requireContext(), 8));
            strip.addView(frame, params);
        }
    }

    private void onPicked(List<Uri> uris) {
        if (uris == null || uris.isEmpty() || getView() == null) return;
        int room = FeedbackApi.MAX_SCREENSHOTS - screenshots.size();
        if (room <= 0) return;
        List<Uri> chosen = uris.size() > room ? uris.subList(0, room) : uris;
        ContentResolver resolver = requireContext().getContentResolver();
        if (uris.size() > room) MsToast.show(requireContext(), "最多附 " + FeedbackApi.MAX_SCREENSHOTS + " 张截图");
        List<Uri> copy = new ArrayList<>(chosen);
        NetworkTask.run(this, () -> {
            List<byte[]> encoded = new ArrayList<>(copy.size());
            for (Uri uri : copy) {
                byte[] bytes = reencode(resolver, uri);
                if (bytes != null) encoded.add(bytes);
            }
            return encoded;
        }, outcome -> {
            List<byte[]> encoded = outcome.value();
            if (encoded == null || encoded.size() < copy.size()) MsToast.show(requireContext(), "有图片读不出来，没有附上");
            if (encoded != null) {
                for (byte[] bytes : encoded) {
                    if (screenshots.size() < FeedbackApi.MAX_SCREENSHOTS) screenshots.add(bytes);
                }
            }
            renderThumbnails();
            refresh();
        });
    }

    /** 读一张图，长边缩到 1600 像素以内，重新编码成 1 MiB 以内的 JPEG；读不出或压不到时返回 null。 */
    @Nullable private static byte[] reencode(ContentResolver resolver, Uri uri) throws IOException {
        byte[] source;
        try (InputStream in = resolver.openInputStream(uri)) {
            source = FeedbackImagePolicy.readSource(in);
        }
        if (source == null) return null;
        BitmapFactory.Options bounds = BitmapPolicy.decodeBounds(source);
        if (bounds == null) return null;
        int sample = BitmapPolicy.sampleSizeForEdge(bounds.outWidth, bounds.outHeight, MAX_EDGE);
        BitmapFactory.Options options = new BitmapFactory.Options();
        options.inSampleSize = sample;
        Bitmap bitmap = BitmapFactory.decodeByteArray(source, 0, source.length, options);
        if (bitmap == null) return null;
        try {
            bitmap = BitmapPolicy.scaleToEdge(bitmap, MAX_EDGE);
            byte[] jpeg = BitmapPolicy.compressJpegUnderBytes(bitmap, FeedbackApi.MAX_SCREENSHOT_BYTES);
            // Rust 不重新编码，超过它的大小或尺寸限制整次提交都会失败；缩不进去的这一张不附。
            return jpeg != null && FeedbackApi.fitsRust(jpeg.length, bitmap.getWidth(), bitmap.getHeight())
                ? jpeg : null;
        } finally {
            bitmap.recycle();
        }
    }

    private void submit() {
        if (sending || sent || !FeedbackApi.validText(draft)) return;
        Context application = requireContext().getApplicationContext();
        sending = true;
        refresh();
        FeedbackApi.Type kind = type;
        String text = draft;
        boolean withDiagnostics = diagnostics;
        List<byte[]> shots = new ArrayList<>(screenshots);
        submitTask = NetworkTask.run(this, () -> {
            List<File> files = new ArrayList<>(shots.size());
            try {
                for (byte[] bytes : shots) files.add(screenshotFile(application, bytes));
                return FeedbackApi.submit(application, kind, text, UpdateJobService.currentVersion(application),
                    AppEdition.current().id(), withDiagnostics ? collectDiagnostics(application) : null, files);
            } finally {
                for (File file : files) FilePolicy.deleteQuietly(file);
            }
        }, outcome -> {
            submitTask = null;
            sending = false;
            if (outcome.error() != null) {
                refresh();
                MsToast.show(requireContext(), failureMessage(outcome.error()));
                return;
            }
            sent = true;
            screenshots.clear();
            renderThumbnails();
            refresh();
            MsToast.show(requireContext(), "已提交，感谢反馈");
        });
    }

    private static String failureMessage(Exception error) {
        if (error instanceof AccountStore.Failure failure)
            return AccountFailureText.feedback(failure.code, failure.retryAfterSeconds);
        return "没有提交成功，请稍后重试";
    }

    /** 把一张已经重新编码的截图写进缓存目录下的 feedback/，交给 Rust 按路径读；调用方提交完就删。 */
    private static File screenshotFile(Context context, byte[] jpeg) throws IOException {
        File directory = new File(context.getCacheDir(), "feedback");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IOException("feedback cache");
        File file = File.createTempFile("screenshot-", ".jpg", directory);
        try (OutputStream output = Files.newOutputStream(file.toPath(), StandardOpenOption.WRITE,
                StandardOpenOption.TRUNCATE_EXISTING, LinkOption.NOFOLLOW_LINKS)) {
            output.write(jpeg);
        } catch (IOException unwritable) {
            FilePolicy.deleteQuietly(file);
            throw unwritable;
        }
        return file;
    }

    /** 白名单里的诊断字段；只有用户打开开关时才调用。 */
    private static Map<String, String> collectDiagnostics(Context context) {
        Map<String, String> values = new LinkedHashMap<>(FeedbackApi.DIAGNOSTIC_KEYS.size());
        values.put("device", Build.MANUFACTURER + " " + Build.MODEL);
        values.put("os", "Android " + Build.VERSION.RELEASE + "（API " + Build.VERSION.SDK_INT + "）");
        values.put("app_version", appVersion(context));
        values.put("edition", AppEdition.current().id());
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences != null) {
            values.put("scheme", preferences.optString("scheme", ""));
            values.put("keyboard_layout", InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", ""));
            values.put("skin", InputViewValuePolicy.textOr(preferences, "screen_keyboard_theme", ""));
        }
        values.put("ime_enabled", String.valueOf(ImeSetup.enabled(context)));
        values.put("ime_default", String.valueOf(ImeSetup.isDefault(context)));
        return FeedbackApi.filterDiagnostics(values);
    }

    private static String appVersion(Context context) {
        try {
            AppVersionPolicy.Version version = AppVersionPolicy.current(context);
            return (version.name() == null ? "—" : version.name()) + " (" + version.code() + ")";
        } catch (PackageManager.NameNotFoundException missing) {
            return "—";
        }
    }

    private void copy(String label, String value, String done) {
        ClipboardActions.copyText(requireContext(), label, value, done);
    }

    /** 没装 Telegram 又没有浏览器时，退而复制链接。 */
    private void openTelegram() {
        if (!LinkActions.open(requireContext(), TELEGRAM_URL)) {
            copy("Telegram", TELEGRAM_URL, "已复制群组链接");
        }
    }

}
