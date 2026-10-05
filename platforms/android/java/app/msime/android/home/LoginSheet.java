package app.msime.android.home;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.content.res.Configuration;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.Matrix;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.RectF;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.net.Uri;
import android.text.InputFilter;
import android.text.InputType;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.method.LinkMovementMethod;
import android.text.style.ClickableSpan;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.core.graphics.PathParser;
import androidx.core.widget.NestedScrollView;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;
import java.util.function.Consumer;

/**
 * 登录面板（M3 modal bottom sheet）：通过 Apple 登录（黑底，深色下白底）、通过 Google 登录（描边）、使用邮箱登录（tonal），点邮箱后原地展开邮箱输入框与「发送验证码」，发出后换成 6 位验证码输入与「登录」。
 *
 * <p>设计写的是「发送登录链接」（魔法链接）；魔法链接需要网站侧的 App Links 配置，这里用后端已上线的 6 位验证码，按钮文案随之改为「发送验证码」。只显示后端接受、且本安装包能用的方式：Apple 要 `/v1/auth/providers` 的 `apple_web` 为真，Google 还要本包带着 server client ID。登录成功不触发同步。
 */
final class LoginSheet {
    /** 用户没登录就关掉了面板时交给调用方的那句话；调用方可以据此不显示任何提示。 */
    static final String CANCELLED = "已取消登录";

    private static final String PRIVACY_URL = "https://msime.app/privacy/";
    private static final String APPLE_PATH = "M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701";
    private static final String[] GOOGLE_PATHS = {
        "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z",
        "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z",
        "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z",
        "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z",
    };
    private static final int[] GOOGLE_COLORS = {0xFFEA4335, 0xFF4285F4, 0xFFFBBC05, 0xFF34A853};
    private static final String MAIL_PATH = "M20 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 14H4V8l8 5 8-5v10zm-8-7L4 6h16l-8 5z";
    private static final String CLOSE_PATH = "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";

    private final Activity activity;
    private final String purpose;
    private final Consumer<String> finished;
    private final BottomSheetDialog dialog;
    private final LinearLayout options;
    private final LinearLayout email;
    private final TextView status;
    private final Consumer<String> appleResult;
    private BackendAccount.EmailChallenge challenge;
    private String emailAddress = "";
    private boolean busy;
    private boolean done;

    /**
     * 先读后端接受的登录方式，再打开面板。从主线程调用；`finished` 在主线程上收到空字符串（已登录）、失败说明或 {@link #CANCELLED}，只收一次。
     *
     * @param purpose `login`，或给已登录账号添加登录方式的 `link`
     */
    static void show(Activity activity, String purpose, Consumer<String> finished) {
        Context context = activity.getApplicationContext();
        new Thread(() -> {
            CloudApi.Providers providers = SignIn.providers(context);
            boolean google = SignIn.google(context, providers);
            activity.runOnUiThread(() -> {
                if (activity.isFinishing() || activity.isDestroyed()) return;
                if (!google && !providers.appleWeb() && !providers.email()) {
                    finished.accept("现在连不上登录服务，请检查网络后再试");
                    return;
                }
                new LoginSheet(activity, purpose, providers, google, finished).dialog.show();
            });
        }, "msime-login-sheet").start();
    }

    private LoginSheet(Activity activity, String purpose, CloudApi.Providers providers, boolean google,
            Consumer<String> finished) {
        this.activity = activity;
        this.purpose = purpose;
        this.finished = finished;
        dialog = new BottomSheetDialog(activity);
        appleResult = this::finishWith;

        LinearLayout root = new LinearLayout(activity);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(24), 0, dp(24), dp(20));
        root.addView(new BottomSheetDragHandleView(activity), new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        LinearLayout header = new LinearLayout(activity);
        header.setOrientation(LinearLayout.HORIZONTAL);
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = new TextView(activity);
        title.setText("link".equals(purpose) ? "添加登录方式" : "登录水杉");
        Ui.style(title, 22, 700, Ui.text(activity));
        title.setAccessibilityHeading(true);
        header.addView(title, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        ImageView close = new ImageView(activity);
        close.setImageDrawable(new PathIcon(24, new String[] {CLOSE_PATH}, new int[] {Ui.text(activity)}));
        close.setPadding(dp(8), dp(8), dp(8), dp(8));
        close.setBackground(Ui.ripple(activity));
        close.setContentDescription("关闭");
        close.setOnClickListener(ignored -> dialog.cancel());
        header.addView(close, new LinearLayout.LayoutParams(dp(40), dp(40)));
        root.addView(header);

        TextView subtitle = new TextView(activity);
        subtitle.setText("在手机、平板和电脑之间同步词库、皮肤和云剪贴板");
        Ui.style(subtitle, 14, 400, Ui.subText(activity));
        root.addView(subtitle, block(2));

        options = new LinearLayout(activity);
        options.setOrientation(LinearLayout.VERTICAL);
        boolean night = (activity.getResources().getConfiguration().uiMode
            & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
        if (providers.appleWeb()) {
            // Apple 的规范按钮：浅色下黑底白字，深色下白底黑字；这是 Apple 的品牌色，不随季节主题变。
            int fill = night ? Color.WHITE : Color.BLACK;
            int ink = night ? Color.BLACK : Color.WHITE;
            options.addView(button(new PathIcon(24, new String[] {APPLE_PATH}, new int[] {ink}),
                "通过 Apple 登录", fill, ink, 0, this::apple), block(16));
        }
        if (google) {
            options.addView(button(new PathIcon(48, GOOGLE_PATHS, GOOGLE_COLORS), "通过 Google 登录",
                Color.TRANSPARENT, Ui.text(activity), Ui.outline(activity), this::google), block(12));
        }
        email = new LinearLayout(activity);
        email.setOrientation(LinearLayout.VERTICAL);
        if (providers.email()) {
            int accent = Ui.accent(activity);
            options.addView(button(new PathIcon(24, new String[] {MAIL_PATH}, new int[] {accent}), "使用邮箱登录",
                Ui.color(activity, com.google.android.material.R.attr.colorSecondaryContainer), accent, 0,
                this::expandEmail), block(12));
            options.addView(email, block(0));
        }
        root.addView(options, block(0));

        status = new TextView(activity);
        Ui.style(status, 13, 400, Ui.subText(activity));
        status.setGravity(Gravity.CENTER_HORIZONTAL);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        status.setVisibility(View.GONE);
        root.addView(status, block(12));

        root.addView(agreement(), block(16));

        NestedScrollView scroll = new NestedScrollView(activity);
        scroll.addView(root, new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT));
        dialog.setContentView(scroll);
        dialog.setOnDismissListener(ignored -> {
            SignIn.stopWaitingForApple(appleResult);
            if (!done) {
                done = true;
                finished.accept(CANCELLED);
            }
        });
    }

    // ---- 三种方式 ----

    private void apple() {
        if (!begin("正在打开 Apple 登录…")) return;
        SignIn.startApple(activity, purpose, appleResult);
        // 浏览器打开后面板留着等回调；用户可以随时关掉它，晚到的回调仍会完成登录。
        busy = false;
        setEnabled(true);
    }

    private void google() {
        if (!begin("正在登录…")) return;
        SignIn.startGoogle(activity, this::finishWith);
    }

    private void expandEmail() {
        if (busy || email.getChildCount() > 0) return;
        challenge = null;
        EditText field = field("邮箱地址", InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS, 254);
        field.setText(emailAddress);
        email.addView(field, block(12));
        View send = button(null, "发送验证码", Ui.accent(activity), Ui.onAccent(activity), 0,
            () -> sendCode(field.getText().toString()));
        email.addView(send, block(12));
        field.setOnEditorActionListener((view, action, event) -> {
            if (action != EditorInfo.IME_ACTION_SEND) return false;
            sendCode(field.getText().toString());
            return true;
        });
        field.setImeOptions(EditorInfo.IME_ACTION_SEND);
        field.requestFocus();
    }

    private void sendCode(String address) {
        if (!begin("正在发送验证码…")) return;
        Context context = activity.getApplicationContext();
        new Thread(() -> {
            SignIn.EmailCode result = SignIn.requestEmailCode(context, address, purpose);
            activity.runOnUiThread(() -> {
                if (done || activity.isFinishing() || activity.isDestroyed()) return;
                busy = false;
                setEnabled(true);
                if (result.challenge() == null) {
                    say(result.failure());
                    return;
                }
                emailAddress = address.trim();
                challenge = result.challenge();
                showCodeEntry();
            });
        }, "msime-email-code").start();
    }

    private void showCodeEntry() {
        email.removeAllViews();
        TextView sent = new TextView(activity);
        sent.setText("验证码已发到 " + emailAddress + "，" + Math.max(1, challenge.expiresIn() / 60) + " 分钟内有效");
        Ui.style(sent, 13, 400, Ui.subText(activity));
        email.addView(sent, block(12));
        EditText code = field("6 位验证码", InputType.TYPE_CLASS_NUMBER, 6);
        code.setImeOptions(EditorInfo.IME_ACTION_DONE);
        code.setLetterSpacing(0.3f);
        email.addView(code, block(8));
        email.addView(button(null, "登录", Ui.accent(activity), Ui.onAccent(activity), 0,
            () -> verify(code.getText().toString())), block(12));
        code.setOnEditorActionListener((view, action, event) -> {
            if (action != EditorInfo.IME_ACTION_DONE) return false;
            verify(code.getText().toString());
            return true;
        });
        TextView again = new TextView(activity);
        again.setText("换个邮箱或重新发送");
        Ui.style(again, 14, 500, Ui.accent(activity));
        again.setGravity(Gravity.CENTER);
        again.setMinHeight(dp(40));
        again.setBackground(Ui.ripple(activity));
        again.setOnClickListener(ignored -> {
            if (busy) return;
            email.removeAllViews();
            status.setVisibility(View.GONE);
            expandEmail();
        });
        email.addView(again, block(4));
        status.setVisibility(View.GONE);
        code.requestFocus();
    }

    private void verify(String code) {
        BackendAccount.EmailChallenge current = challenge;
        if (current == null || !begin("正在登录…")) return;
        Context context = activity.getApplicationContext();
        new Thread(() -> {
            String failure = SignIn.verifyEmailCode(context, current, code);
            activity.runOnUiThread(() -> finishWith(failure));
        }, "msime-email-login").start();
    }

    // ---- 状态 ----

    private boolean begin(String message) {
        if (busy || done) return false;
        busy = true;
        setEnabled(false);
        say(message);
        return true;
    }

    /** 一种方式结束：成功就关面板并交回空字符串；失败就把原因写在面板上，让人换一种方式或再试一次。 */
    private void finishWith(String failure) {
        if (done || activity.isFinishing() || activity.isDestroyed()) return;
        busy = false;
        setEnabled(true);
        if (!failure.isEmpty()) {
            say(failure);
            return;
        }
        done = true;
        finished.accept("");
        dialog.dismiss();
    }

    private void say(String message) {
        status.setText(message);
        status.setVisibility(message.isEmpty() ? View.GONE : View.VISIBLE);
    }

    private void setEnabled(boolean enabled) {
        setEnabled(options, enabled);
    }

    private static void setEnabled(ViewGroup group, boolean enabled) {
        for (int index = 0; index < group.getChildCount(); index++) {
            View child = group.getChildAt(index);
            if (child instanceof ViewGroup nested && !child.isClickable()) {
                setEnabled(nested, enabled);
            } else {
                child.setEnabled(enabled);
                child.setAlpha(enabled ? 1f : 0.6f);
            }
        }
    }

    // ---- 部件 ----

    /** 50dp 高、12dp 圆角的整行按钮：可选的 20dp 图标加 16sp 半粗文字，居中。 */
    private View button(Drawable icon, String label, int fill, int ink, int stroke, Runnable action) {
        LinearLayout button = new LinearLayout(activity);
        button.setOrientation(LinearLayout.HORIZONTAL);
        button.setGravity(Gravity.CENTER);
        button.setMinimumHeight(dp(50));
        GradientDrawable face = Ui.rounded(fill, dp(12));
        if (stroke != 0) face.setStroke(Math.max(1, dp(1)), stroke);
        GradientDrawable mask = Ui.rounded(Color.WHITE, dp(12));
        int pressed = Ui.withAlpha(fill == Color.BLACK ? Color.WHITE : Ui.text(activity), 0.12f);
        button.setBackground(new RippleDrawable(android.content.res.ColorStateList.valueOf(pressed), face, mask));
        if (icon != null) {
            ImageView glyph = new ImageView(activity);
            glyph.setImageDrawable(icon);
            glyph.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(dp(20), dp(20));
            params.setMarginEnd(dp(8));
            button.addView(glyph, params);
        }
        TextView text = new TextView(activity);
        text.setText(label);
        Ui.style(text, 16, 600, ink);
        button.addView(text);
        button.setContentDescription(label);
        button.setClickable(true);
        button.setFocusable(true);
        button.setOnClickListener(ignored -> action.run());
        return button;
    }

    private EditText field(String hint, int inputType, int maxLength) {
        EditText field = new EditText(activity);
        field.setHint(hint);
        field.setInputType(inputType);
        field.setSingleLine(true);
        field.setFilters(new InputFilter[] {new InputFilter.LengthFilter(maxLength)});
        Ui.style(field, 16, 400, Ui.text(activity));
        field.setHintTextColor(Ui.subText(activity));
        GradientDrawable face = Ui.rounded(Ui.rowBackground(activity), dp(12));
        face.setStroke(Math.max(1, dp(1)), Ui.hairline(activity));
        field.setBackground(face);
        field.setPadding(dp(14), 0, dp(14), 0);
        field.setMinHeight(dp(50));
        field.setGravity(Gravity.CENTER_VERTICAL);
        field.setContentDescription(hint);
        return field;
    }

    private TextView agreement() {
        TextView view = new TextView(activity);
        String text = "登录即表示同意《用户协议》和《隐私政策》";
        SpannableString spanned = new SpannableString(text);
        int start = text.indexOf("《隐私政策》");
        spanned.setSpan(new ClickableSpan() {
            @Override public void onClick(View widget) {
                try {
                    activity.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(PRIVACY_URL)));
                } catch (ActivityNotFoundException absent) {
                    say("这台设备上没有可以打开网页的浏览器");
                }
            }

            @Override public void updateDrawState(android.text.TextPaint paint) {
                paint.setColor(Ui.subText(activity));
                paint.setUnderlineText(false);
            }
        }, start, start + "《隐私政策》".length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        view.setText(spanned);
        view.setMovementMethod(LinkMovementMethod.getInstance());
        Ui.style(view, 12, 400, Ui.subText(activity));
        view.setGravity(Gravity.CENTER_HORIZONTAL);
        return view;
    }

    private LinearLayout.LayoutParams block(int top) {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = dp(top);
        return params;
    }

    private int dp(int value) {
        return Ui.dp(activity, value);
    }

    /** 按 SVG path 数据画的图标：每条 path 一种颜色，按 `viewport` 等比缩放到边界里。 */
    private static final class PathIcon extends Drawable {
        private final float viewport;
        private final Path[] paths;
        private final int[] colors;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Path scaled = new Path();
        private final Matrix matrix = new Matrix();

        PathIcon(float viewport, String[] data, int[] colors) {
            this.viewport = viewport;
            this.paths = new Path[data.length];
            for (int index = 0; index < data.length; index++) paths[index] = PathParser.createPathFromPathData(data[index]);
            this.colors = colors;
            paint.setStyle(Paint.Style.FILL);
        }

        @Override public void draw(Canvas canvas) {
            RectF bounds = new RectF(getBounds());
            float scale = Math.min(bounds.width(), bounds.height()) / viewport;
            matrix.setScale(scale, scale);
            matrix.postTranslate(bounds.left + (bounds.width() - viewport * scale) / 2f,
                bounds.top + (bounds.height() - viewport * scale) / 2f);
            for (int index = 0; index < paths.length; index++) {
                scaled.reset();
                paths[index].transform(matrix, scaled);
                paint.setColor(colors[Math.min(index, colors.length - 1)]);
                canvas.drawPath(scaled, paint);
            }
        }

        @Override public void setAlpha(int alpha) { paint.setAlpha(alpha); }

        @Override public void setColorFilter(ColorFilter filter) { paint.setColorFilter(filter); }

        @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
    }
}
