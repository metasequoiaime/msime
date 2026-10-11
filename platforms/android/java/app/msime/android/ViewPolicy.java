package app.msime.android;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Paint;
import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.graphics.Typeface;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.util.TypedValue;
import android.text.TextUtils;
import android.text.Editable;
import android.text.TextWatcher;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.util.function.Consumer;
import android.widget.PopupWindow;

/** Shared view configuration for host controls whose widget defaults need resetting. */
public final class ViewPolicy {
    private static final float TRAILING_VALUE_MAX_SHARE = 0.5f;
    private ViewPolicy() {}

    /** Remove both legacy and platform minimum-height constraints from a view. */
    public static void clearMinimumHeight(View view) {
        view.setMinimumHeight(0);
    }

    /** Remove both minimum-height constraints from a text widget. */
    public static void clearMinimumHeight(TextView view) {
        view.setMinHeight(0);
        view.setMinimumHeight(0);
    }

    /** Apply shared text size, weight and colour styling to a text view. */
    public static void style(TextView view, int sizeSp, int weight, int color) {
        setTextSizeSp(view, sizeSp);
        setTypefaceWeight(view, weight);
        setTextColor(view, color);
    }

    /** Create a text label with the shared size, weight, and colour policy. */
    public static TextView styledLabel(Context context, CharSequence text, int sizeSp,
                                       int weight, int color) {
        TextView view = newTextView(context, text);
        style(view, sizeSp, weight, color);
        return view;
    }

    /** Create a styled text label constrained to one line. */
    public static TextView singleLineLabel(Context context, CharSequence text, int sizeSp,
                                           int weight, int color) {
        TextView view = newTextView(context, text);
        style(view, sizeSp, weight, color);
        setSingleLine(view);
        return view;
    }

    /** Create a text label centered on both axes. */
    public static TextView centeredLabel(Context context, CharSequence text, float sizeSp,
                                         int color) {
        TextView view = label(context, text, sizeSp, color);
        setCentered(view);
        return view;
    }

    /** Create a styled text label centered on both axes. */
    public static TextView centeredLabel(Context context, CharSequence text, int sizeSp,
                                         int weight, int color) {
        TextView view = newTextView(context, text);
        style(view, sizeSp, weight, color);
        setCentered(view);
        return view;
    }

    /** Create a text label carrying accessibility heading semantics. */
    public static TextView headingLabel(Context context, CharSequence text, float sizeSp,
                                        int color) {
        TextView view = label(context, text, sizeSp, color);
        view.setAccessibilityHeading(true);
        return view;
    }

    /** Create a styled text label carrying accessibility heading semantics. */
    public static TextView headingLabel(Context context, CharSequence text, int sizeSp,
                                        int weight, int color) {
        TextView view = newTextView(context, text);
        style(view, sizeSp, weight, color);
        view.setAccessibilityHeading(true);
        return view;
    }

    /** Create a centered styled text label constrained to one line. */
    public static TextView centeredSingleLineLabel(Context context, CharSequence text, int sizeSp,
                                                   int weight, int color) {
        TextView view = centeredLabel(context, text, sizeSp, weight, color);
        setSingleLine(view);
        return view;
    }

    /** Create a polite live-region status label for accessibility announcements. */
    public static TextView liveStatus(Context context, int sizeSp, int color) {
        TextView view = newTextView(context, "");
        style(view, sizeSp, 400, color);
        setPoliteLiveRegion(view);
        return view;
    }

    /** Create an editable field with the shared size, weight, and colour policy. */
    public static EditText styledInput(Context context, int sizeSp, int weight, int color) {
        EditText view = new EditText(context);
        style(view, sizeSp, weight, color);
        return view;
    }

    /** Create a centered, clickable text button with caller-supplied background and ink. */
    public static TextView textButton(Context context, CharSequence label, int sizeSp, int weight,
                                      int ink, Drawable background, float minHeightDp) {
        return textButton(context, label, sizeSp, weight, ink, background, minHeightDp, null);
    }

    /** Create a centered text button and bind its action. */
    public static TextView textButton(Context context, CharSequence label, int sizeSp, int weight,
                                      int ink, Drawable background, float minHeightDp,
                                      Runnable action) {
        TextView button = centeredLabel(context, label, sizeSp, weight, ink);
        setBackground(button, background);
        setTextMinHeight(button, DimensionPolicy.pixels(context, minHeightDp));
        setInteractive(button, true);
        bindOptionalClick(button, action);
        return button;
    }

    /** Create a filled accent pill button with caller-selected dimensions. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      int fill, int ink, float horizontalPaddingDp,
                                      float verticalPaddingDp, float minHeightDp, float minWidthDp) {
        return pillButton(context, label, sizeSp, weight, fill, ink, horizontalPaddingDp,
            verticalPaddingDp, minHeightDp, minWidthDp, null);
    }

    /** Create a filled pill button and bind its optional action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      int fill, int ink, float horizontalPaddingDp,
                                      float verticalPaddingDp, float minHeightDp, float minWidthDp,
                                      Runnable action) {
        TextView button = centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);
        setBackground(button, pillRipple(context, fill));
        int horizontal = DimensionPolicy.pixels(context, horizontalPaddingDp);
        int vertical = DimensionPolicy.pixels(context, verticalPaddingDp);
        setPadding(button, horizontal, vertical, horizontal, vertical);
        setTextMinHeight(button, DimensionPolicy.pixels(context, minHeightDp));
        if (minWidthDp > 0) setTextMinWidth(button, DimensionPolicy.pixels(context, minWidthDp));
        setInteractive(button, true);
        bindOptionalClick(button, action);
        return button;
    }

    /** Create a single-line trailing value capped to half of its available row width. */
    public static TextView trailingValue(Context context, CharSequence text, int sizeSp, int color) {
        TextView view = new TextView(context) {
            @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
                if (MeasureSpec.getMode(widthMeasureSpec) != MeasureSpec.UNSPECIFIED) {
                    int limit = Math.round(MeasureSpec.getSize(widthMeasureSpec) * TRAILING_VALUE_MAX_SHARE);
                    widthMeasureSpec = MeasureSpec.makeMeasureSpec(limit, MeasureSpec.AT_MOST);
                }
                super.onMeasure(widthMeasureSpec, heightMeasureSpec);
            }
        };
        view.setText(text);
        style(view, sizeSp, 400, color);
        setSingleLineEllipsized(view);
        return view;
    }

    /** Apply a minimum height to a generic view. */
    public static void setMinimumHeight(View view, int height) {
        view.setMinimumHeight(height);
    }

    /** Remove both minimum width and height constraints from a generic view. */
    public static void clearMinimumSize(View view) {
        clearMinimumWidth(view);
        clearMinimumHeight(view);
    }

    /** Remove both minimum width and height constraints from a text widget. */
    public static void clearMinimumSize(TextView view) {
        clearMinimumWidth(view);
        clearMinimumHeight(view);
    }

    /** Apply a minimum height to both text-widget constraints. */
    public static void setMinimumHeight(TextView view, int height) {
        view.setMinHeight(height);
        view.setMinimumHeight(height);
    }

    /** 应用文本控件需要的按行最小高度。 */
    public static void setTextMinHeight(TextView view, int height) {
        setMinimumHeight(view, height);
    }

    /** Remove both legacy and platform minimum-width constraints from a view. */
    public static void clearMinimumWidth(View view) {
        view.setMinimumWidth(0);
    }

    /** Remove both minimum-width constraints from a text widget. */
    public static void clearMinimumWidth(TextView view) {
        view.setMinWidth(0);
        view.setMinimumWidth(0);
    }

    /** Apply a minimum width to a generic view. */
    public static void setMinimumWidth(View view, int width) {
        view.setMinimumWidth(width);
    }

    /** Apply a minimum width to both text-widget constraints. */
    public static void setMinimumWidth(TextView view, int width) {
        view.setMinWidth(width);
        view.setMinimumWidth(width);
    }

    /** 应用文本控件需要的按行最小宽度。 */
    public static void setTextMinWidth(TextView view, int width) {
        setMinimumWidth(view, width);
    }

    /** Apply equal horizontal and vertical padding to a view. */
    public static void setSymmetricPadding(View view, int horizontal, int vertical) {
        view.setPadding(horizontal, vertical, horizontal, vertical);
    }

    /** Apply the same pixel inset on all four sides. */
    public static void setSymmetricPadding(View view, int padding) {
        setSymmetricPadding(view, padding, padding);
    }

    /** Apply equal padding expressed in density-independent pixels. */
    public static void setSymmetricPaddingDp(Context context, View view, float horizontalDp,
                                             float verticalDp) {
        setSymmetricPadding(view, DimensionPolicy.pixels(context, horizontalDp),
            DimensionPolicy.pixels(context, verticalDp));
    }

    /** Apply equal horizontal pixel padding while leaving vertical padding unset. */
    public static void setHorizontalPadding(View view, int horizontal) {
        view.setPadding(horizontal, 0, horizontal, 0);
    }

    /** Apply equal horizontal padding expressed in density-independent pixels. */
    public static void setHorizontalPaddingDp(Context context, View view, float horizontalDp) {
        setHorizontalPadding(view, DimensionPolicy.pixels(context, horizontalDp));
    }

    /** Apply four-sided padding expressed in density-independent pixels. */
    public static void setPaddingDp(Context context, View view, float leftDp, float topDp,
                                    float rightDp, float bottomDp) {
        setPadding(view, DimensionPolicy.pixels(context, leftDp), DimensionPolicy.pixels(context, topDp),
            DimensionPolicy.pixels(context, rightDp), DimensionPolicy.pixels(context, bottomDp));
    }

    /** Clear all view padding. */
    public static void clearPadding(View view) {
        view.setPadding(0, 0, 0, 0);
    }

    /** Clear vertical padding while preserving horizontal padding. */
    public static void clearVerticalPadding(View view) {
        view.setPadding(view.getPaddingLeft(), 0, view.getPaddingRight(), 0);
    }

    /** Set equal horizontal padding while preserving the current vertical padding. */
    public static void setHorizontalPaddingPreservingVertical(View view, int horizontal) {
        view.setPadding(horizontal, view.getPaddingTop(), horizontal, view.getPaddingBottom());
    }

    /** 只替换底部内边距，保留其余三边。 */
    public static void setBottomPadding(View view, int bottom) {
        setPadding(view, view.getPaddingLeft(), view.getPaddingTop(), view.getPaddingRight(), bottom);
    }

    /** Apply explicit pixel padding on all four sides. */
    public static void setPadding(View view, int left, int top, int right, int bottom) {
        view.setPadding(left, top, right, bottom);
    }

    /** 仅在四边内边距发生变化时更新视图，避免无意义的重新布局。 */
    public static void setPaddingIfChanged(View view, int left, int top, int right, int bottom) {
        if (view.getPaddingLeft() == left && view.getPaddingTop() == top
                && view.getPaddingRight() == right && view.getPaddingBottom() == bottom) return;
        setPadding(view, left, top, right, bottom);
    }

    /** Remove the default background, padding, and minimum size from a view. */
    public static void clearChrome(View view) {
        clearBackground(view);
        clearPadding(view);
        clearMinimumSize(view);
    }

    /** Keep a button label in its authored casing instead of applying the platform default. */
    public static void setAllCapsFalse(Button button) {
        button.setAllCaps(false);
    }

    /** Create a text view with its initial content assigned. */
    public static TextView label(Context context, CharSequence text, float sizeSp, int color) {
        TextView view = newTextView(context, text);
        setTextSizeSp(view, sizeSp);
        setTextColor(view, color);
        return view;
    }

    /** Create a text view with its initial content assigned. */
    public static TextView newTextView(Context context, CharSequence text) {
        TextView view = new TextView(context);
        view.setText(text);
        return view;
    }

    /** 创建使用指定纯色背景的视图。 */
    public static View newColorView(Context context, int color) {
        View view = new View(context);
        setBackgroundColor(view, color);
        return view;
    }

    /** Create a keyboard press button with authored casing preserved. */
    public static KeyboardPressButton newPressButton(android.content.Context context) {
        KeyboardPressButton button = new KeyboardPressButton(context);
        setAllCapsFalse(button);
        return button;
    }

    /** 创建横向排列的线性容器。 */
    public static LinearLayout newRow(Context context) {
        LinearLayout view = new LinearLayout(context);
        view.setOrientation(LinearLayout.HORIZONTAL);
        return view;
    }

    /** 创建纵向排列的线性容器。 */
    public static LinearLayout newColumn(Context context) {
        LinearLayout view = new LinearLayout(context);
        view.setOrientation(LinearLayout.VERTICAL);
        return view;
    }

    /** 创建像素尺寸的线性布局正方形参数。 */
    public static LinearLayout.LayoutParams newSquareParamsPx(int size) {
        return new LinearLayout.LayoutParams(size, size);
    }

    /** Invoke a listener only after text changes, without requiring unused watcher callbacks. */
    public static void afterTextChanged(TextView view, Consumer<Editable> listener) {
        view.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) { listener.accept(text); }
        });
    }

    /** Bind a caller-supplied action to a view without changing any other interaction policy. */
    public static void bindClick(View view, Runnable action) {
        view.setOnClickListener(ignored -> action.run());
    }

    /** Bind an optional action, clearing the listener when no action is available. */
    public static void bindOptionalClick(View view, Runnable action) {
        view.setOnClickListener(action == null ? null : ignored -> action.run());
    }

    /** Resolve the theme's standard selectable-item ripple. */
    public static Drawable ripple(Context context) {
        TypedValue value = new TypedValue();
        if (!context.getTheme().resolveAttribute(android.R.attr.selectableItemBackground, value, true)) {
            return null;
        }
        return context.getDrawable(value.resourceId);
    }

    /** Resolve a theme ripple over a caller-supplied rounded fill and mask. */
    public static Drawable ripple(Context context, int fill, float radiusPx) {
        int pressed = ColorPolicy.withAlpha(primaryTextColor(context), 0.10f);
        return new RippleDrawable(ColorStateList.valueOf(pressed), rounded(fill, radiusPx),
            rounded(Color.WHITE, radiusPx));
    }

    private static GradientDrawable rounded(int color, float radiusPx) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setColor(color);
        shape.setCornerRadius(radiusPx);
        return shape;
    }

    /** Resolve the framework primary text colour without depending on the Material host layer. */
    private static int primaryTextColor(Context context) {
        TypedValue value = new TypedValue();
        if (!context.getTheme().resolveAttribute(android.R.attr.textColorPrimary, value, true)) {
            return Color.BLACK;
        }
        if (value.type >= TypedValue.TYPE_FIRST_COLOR_INT && value.type <= TypedValue.TYPE_LAST_COLOR_INT) {
            return value.data;
        }
        try {
            return context.getColorStateList(value.resourceId).getDefaultColor();
        } catch (RuntimeException ignored) {
            return Color.BLACK;
        }
    }

    /** Resolve a theme ripple over a fully rounded fill. */
    public static Drawable pillRipple(Context context, int fill) {
        return ripple(context, fill, 9999f);
    }

    /** Apply the standard theme ripple and optional click behavior to a view. */
    public static void makeClickable(View view, Context context, Runnable action) {
        setBackground(view, ViewPolicy.ripple(context));
        setInteractive(view, true);
        bindOptionalClick(view, action);
    }

    /** Set whether a view accepts input without changing its visibility or focus policy. */
    public static void setEnabled(View view, boolean enabled) {
        view.setEnabled(enabled);
    }

    /** Apply enabled state and a caller-selected inactive opacity. */
    public static void setEnabledWithAlpha(View view, boolean enabled, float inactiveAlpha) {
        setEnabled(view, enabled);
        setActiveAlpha(view, enabled, inactiveAlpha);
    }

    /** 递归设置未承载点击行为的容器及其控件的启用状态和禁用透明度。 */
    public static void setEnabledRecursively(ViewGroup group, boolean enabled, float inactiveAlpha) {
        for (int index = 0; index < group.getChildCount(); index++) {
            View child = group.getChildAt(index);
            if (child instanceof ViewGroup nested && !child.isClickable()) {
                setEnabledRecursively(nested, enabled, inactiveAlpha);
            } else {
                setEnabledWithAlpha(child, enabled, inactiveAlpha);
            }
        }
    }

    /** 设置视图的固定像素高度；布局参数缺失或高度未变化时不触发重新布局。 */
    public static void setFixedHeight(View view, int height) {
        android.view.ViewGroup.LayoutParams params = view.getLayoutParams();
        if (params == null || params.height == height) return;
        params.height = height;
        view.setLayoutParams(params);
    }

    /** 判断视图是否非空且正在显示。 */
    public static boolean isVisible(View view) {
        return view != null && view.getVisibility() == View.VISIBLE;
    }

    /** 判断视图是否非空且已从布局中隐藏。null 返回 false，所以 {@code !isGone(view)} 不代表视图非空；hide、show 等改可见性的方法不接受 null，接着调用前调用方要自己判空。 */
    public static boolean isGone(View view) {
        return view != null && view.getVisibility() == View.GONE;
    }

    /** Announce changing view content to accessibility services without interrupting the user. */
    public static void setPoliteLiveRegion(View view) {
        view.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
    }

    /** Center a view's content on both axes. */
    public static void setCentered(View view) {
        if (view instanceof TextView text) {
            text.setGravity(Gravity.CENTER);
        } else if (view instanceof LinearLayout layout) {
            layout.setGravity(Gravity.CENTER);
        } else {
            throw new IllegalArgumentException("Centered policy requires a text or linear-layout view");
        }
    }

    /** Center a view's content along the vertical axis. */
    public static void setCenteredVertically(View view) {
        if (view instanceof TextView text) {
            text.setGravity(Gravity.CENTER_VERTICAL);
        } else if (view instanceof LinearLayout layout) {
            layout.setGravity(Gravity.CENTER_VERTICAL);
        } else {
            throw new IllegalArgumentException("Vertical centering requires a text or linear-layout view");
        }
    }

    /** Center a view's content along the horizontal axis. */
    public static void setCenteredHorizontally(LinearLayout view) {
        view.setGravity(Gravity.CENTER_HORIZONTAL);
    }

    /** Set an explicit child gravity on a linear layout. */
    public static void setGravity(LinearLayout view, int gravity) {
        view.setGravity(gravity);
    }

    /** Center a text view's content along the horizontal axis. */
    public static void setCenteredHorizontally(TextView view) {
        view.setGravity(Gravity.CENTER_HORIZONTAL);
    }

    /** Align a view's content to the start edge and center it vertically. */
    public static void setStartCenteredVertically(LinearLayout view) {
        view.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
    }

    /** Align a text view's content to the start edge and center it vertically. */
    public static void setStartCenteredVertically(TextView view) {
        view.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
    }

    /** Align a view's content to the end edge and center it vertically. */
    public static void setEndCenteredVertically(TextView view) {
        view.setGravity(Gravity.END | Gravity.CENTER_VERTICAL);
    }

    /** Align a multi-line text field to the start edge and top. */
    public static void setTopStart(TextView view) {
        view.setGravity(Gravity.TOP | Gravity.START);
    }

    /** Apply a typeface style while preserving the text view's current family. */
    public static void setTypefaceStyle(TextView view, int style) {
        view.setTypeface(view.getTypeface(), style);
    }

    /** 直接设置文本视图的字体。 */
    public static void setTypeface(TextView view, Typeface typeface) {
        view.setTypeface(typeface);
    }

    /** 直接设置文本画笔的字体。 */
    public static void setTypeface(Paint paint, Typeface typeface) {
        paint.setTypeface(typeface);
    }

    /** 直接设置文本视图的字体和字形样式。 */
    public static void setTypeface(TextView view, Typeface typeface, int style) {
        view.setTypeface(typeface, style);
    }

    /** Apply a default-family typeface with the supplied numeric weight. */
    public static void setTypefaceWeight(TextView view, int weight) {
        view.setTypeface(Typeface.create(Typeface.DEFAULT, weight, false));
    }

    /** Set a text view's size in scalable pixels. */
    public static void setTextSizeSp(TextView view, float sizeSp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp);
    }

    /** Set a paint's text size in scalable pixels using the supplied display context. */
    public static void setTextSizeSp(Paint paint, Context context, float sizeSp) {
        paint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sizeSp,
            context.getResources().getDisplayMetrics()));
    }

    /** 直接设置文本画笔的像素字号。 */
    public static void setTextSize(Paint paint, float sizePx) {
        paint.setTextSize(sizePx);
    }

    /** 直接设置文本视图的像素字号。 */
    public static void setTextSize(TextView view, float sizePx) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_PX, sizePx);
    }

    /** Set a text view's size in density-independent pixels. */
    public static void setTextSizeDp(TextView view, float sizeDp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_DIP, sizeDp);
    }

    /** Set a text view's solid foreground color. */
    public static void setTextColor(TextView view, int color) {
        view.setTextColor(color);
    }

    /** Set a text view's state-aware foreground colors. */
    public static void setTextColor(TextView view, ColorStateList colors) {
        view.setTextColor(colors);
    }

    /** Set a text label and its scalable size. */
    public static void setTextSizeLabel(TextView view, CharSequence text, float sizeSp) {
        view.setText(text);
        setTextSizeSp(view, sizeSp);
    }

    /** Create a text label with its scalable size assigned. */
    public static TextView textLabel(Context context, CharSequence text, float sizeSp) {
        TextView view = newTextView(context, text);
        setTextSizeSp(view, sizeSp);
        return view;
    }

    /** Set a text view's scalable size and center its content. */
    public static void setCenteredTextSizeSp(TextView view, float sizeSp) {
        setTextSizeSp(view, sizeSp);
        setCentered(view);
    }

    /** Set a keyboard-scaled size and center the text view's content. */
    public static void setCenteredKeyTextSizeSp(TextView view, float sizeSp) {
        KeyboardTypographyPolicy.setKeyTextSize(view, sizeSp);
        setCentered(view);
    }

    /** Set a centered text label and its scalable size. */
    public static void setCenteredText(TextView view, CharSequence text, float sizeSp) {
        view.setText(text);
        setCenteredTextSizeSp(view, sizeSp);
    }

    /** Create a centered text label with its scalable size assigned. */
    public static TextView centeredText(Context context, CharSequence text, float sizeSp) {
        TextView view = newTextView(context, text);
        setCenteredTextSizeSp(view, sizeSp);
        return view;
    }

    /** Set a text view's scalable size and align its content to the start edge vertically centered. */
    public static void setStartCenteredTextSizeSp(TextView view, float sizeSp) {
        setTextSizeSp(view, sizeSp);
        setStartCenteredVertically(view);
    }

    /** Set a keyboard-scaled size and align its content to the start edge vertically centered. */
    public static void setStartCenteredKeyTextSizeSp(TextView view, float sizeSp) {
        KeyboardTypographyPolicy.setKeyTextSize(view, sizeSp);
        setStartCenteredVertically(view);
    }

    /** Set a view background while preserving its other visual state. */
    public static void setBackground(View view, android.graphics.drawable.Drawable background) {
        view.setBackground(background);
    }

    /** Apply a solid background color while preserving the view's other visual state. */
    public static void setBackgroundColor(View view, int color) {
        view.setBackgroundColor(color);
    }

    /** Remove a view's default background drawable. */
    public static void clearBackground(View view) {
        setBackground(view, null);
    }

    /** Limit a text view to a fixed number of lines and truncate at the end. */
    public static void setMaxLinesEllipsized(TextView view, int maxLines) {
        view.setMaxLines(maxLines);
        view.setEllipsize(TextUtils.TruncateAt.END);
    }

    /** Keep a text view on one line and truncate overflowing text at the end. */
    public static void setSingleLineEllipsized(TextView view) {
        view.setSingleLine(true);
        view.setEllipsize(TextUtils.TruncateAt.END);
    }

    /** Keep a text view on one line without changing its truncation policy. */
    public static void setSingleLine(TextView view) {
        view.setSingleLine(true);
    }

    /** Set whether a text view is constrained to one line without changing truncation policy. */
    public static void setSingleLine(TextView view, boolean singleLine) {
        view.setSingleLine(singleLine);
    }

    /** Set additional line spacing and multiplier on a text view. */
    public static void setLineSpacing(TextView view, float add, float multiplier) {
        view.setLineSpacing(add, multiplier);
    }

    /** Limit a text view to a maximum number of lines without changing truncation policy. */
    public static void setMaxLines(TextView view, int maxLines) {
        view.setMaxLines(maxLines);
    }

    /** Require a text view to occupy at least the requested number of lines. */
    public static void setMinLines(TextView view, int minLines) {
        view.setMinLines(minLines);
    }

    /** Force a text view to occupy exactly the requested number of lines. */
    public static void setFixedLines(TextView view, int lines) {
        view.setMinLines(lines);
        view.setMaxLines(lines);
    }

    /** Configure uniform automatic text sizing with scalable-pixel bounds. */
    public static void setAutoSizeSp(TextView view, int minSp, int maxSp, int stepSp) {
        view.setAutoSizeTextTypeUniformWithConfiguration(minSp, maxSp, stepSp,
            TypedValue.COMPLEX_UNIT_SP);
    }

    /** Remove Android's extra font top and bottom padding from a text view. */
    public static void clearFontPadding(TextView view) {
        view.setIncludeFontPadding(false);
    }

    /** Remove font padding and vertical view padding while preserving horizontal padding. */
    public static void clearFontAndVerticalPadding(TextView view) {
        clearFontPadding(view);
        clearVerticalPadding(view);
    }

    /** Make a view passive for touch and focus navigation. */
    public static void setNonInteractive(View view) {
        view.setClickable(false);
        view.setFocusable(false);
    }

    /** Expose a view's selected state to drawable and accessibility state lists. */
    public static void setSelected(View view, boolean selected) {
        view.setSelected(selected);
    }

    /** Set whether a view participates in touch and focus navigation. */
    public static void setInteractive(View view, boolean interactive) {
        view.setClickable(interactive);
        view.setFocusable(interactive);
    }

    /** Set whether a view responds to taps without changing its focus policy. */
    public static void setClickable(View view, boolean clickable) {
        view.setClickable(clickable);
    }

    /** Set whether a view accepts keyboard focus without changing its click policy. */
    public static void setFocusable(View view, boolean focusable) {
        view.setFocusable(focusable);
    }

    /** Exclude a decorative view from the accessibility tree. */
    public static void hideFromAccessibility(View view) {
        view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    /** Set the platform accessibility importance policy for a view. */
    public static void setImportantForAccessibility(View view, int mode) {
        view.setImportantForAccessibility(mode);
    }

    /** Remove the platform state-list animator from a view. */
    public static void clearStateListAnimator(View view) {
        view.setStateListAnimator(null);
    }

    /** Remove any platform elevation from a view. */
    public static void clearElevation(View view) {
        view.setElevation(0);
    }

    /** 设置视图的浮层高度。 */
    public static void setElevation(View view, float elevation) {
        view.setElevation(elevation);
    }

    /** 设置弹出窗口的浮层高度。 */
    public static void setElevation(PopupWindow popup, float elevation) {
        popup.setElevation(elevation);
    }

    /** Apply full opacity to an active view and a caller-selected opacity otherwise. */
    public static void setActiveAlpha(View view, boolean active, float inactiveAlpha) {
        view.setAlpha(active ? 1f : inactiveAlpha);
    }

    /** Apply minimum width and height constraints to a view. */
    public static void setMinimumSize(View view, int width, int height) {
        view.setMinimumWidth(width);
        view.setMinimumHeight(height);
    }

    /** Show a view in layout and rendering. */
    public static void show(View view) {
        view.setVisibility(View.VISIBLE);
    }

    /** Hide a view from layout and rendering. */
    public static void hide(View view) {
        view.setVisibility(View.GONE);
    }

    /** Toggle between visible and gone layout participation. */
    public static void setVisible(View view, boolean visible) {
        view.setVisibility(visible ? View.VISIBLE : View.GONE);
    }

    /** 仅在显示状态发生变化时更新视图，避免无意义的重新布局。 */
    public static void setVisibleIfChanged(View view, boolean visible) {
        int visibility = visible ? View.VISIBLE : View.GONE;
        if (view.getVisibility() == visibility) return;
        setVisible(view, visible);
    }

    /** Show a view only when the supplied text is non-null and non-empty. */
    public static void setVisibilityForText(View view, CharSequence text) {
        setVisible(view, text != null && text.length() != 0);
    }

    /** Hide a view while preserving its layout space. */
    public static void setInvisible(View view) {
        view.setVisibility(View.INVISIBLE);
    }
}
