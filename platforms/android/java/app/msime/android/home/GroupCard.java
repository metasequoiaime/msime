package app.msime.android.home;

import android.content.Context;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.Switch;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import java.util.function.IntFunction;

/**
 * 详情页的一组设置：上面一个组标题，下面一张 r16 的 andCard 卡片，卡片里一行行设置。
 *
 * <p>组标题是强调色 14sp/500，相对卡片缩进 8dp，与卡片间隔 2dp；组与组之间 20dp。行最少 64dp 高、左右 16、上下 8，标题 16sp，副标题 14sp 次要文字色，行尾控件与文字间隔 14。行的种类就是设计里出现的这几种：导航（行尾是值和 ›）、值（只显示）、开关、滑块、按钮（tonal 胶囊，原型里漏出来的绿色底在这里一律是 accentSoft）和说明文字。
 *
 * <p>设计稿里普通分组是直角的 rowBg 色块、语言卡才是 r16 的 andCard，这是原型的不一致；按设计令牌的建议统一成 r16 andCard。颜色全部取主题属性。
 */
public final class GroupCard {
    private final Context context;
    private final LinearLayout group;
    private final LinearLayout card;
    private int dividerInset = -1;

    private GroupCard(ViewGroup parent, @Nullable CharSequence title) {
        context = parent.getContext();
        group = Ui.column(context);
        LinearLayout.LayoutParams groupParams = Ui.matchWidth();
        if (parent.getChildCount() > 0) groupParams.topMargin = Ui.dp(context, Ui.GROUP_GAP);

        if (title != null && title.length() > 0) {
            TextView heading = Ui.groupHeading(context, title);
            Ui.setPaddingDp(heading, context, Ui.GROUP_TITLE_INSET, 0,
                Ui.GROUP_TITLE_INSET, 2);
            LinearLayout.LayoutParams params = Ui.matchWidth();
            params.bottomMargin = Ui.dp(context, 2);
            group.addView(heading, params);
        }

        card = Ui.verticalCard(context, Ui.GROUP_RADIUS);
        // 行的按压波纹裁在卡片圆角里，首尾两行不会露出直角。
        card.setClipToOutline(true);
        group.addView(card, Ui.matchWidth());
        parent.addView(group, groupParams);
    }

    /** 在 `parent`（详情页的内容列）末尾加一组；`title` 为空时只有卡片。 */
    public static GroupCard add(ViewGroup parent, @Nullable CharSequence title) {
        return new GroupCard(parent, title);
    }

    /** 整组（标题加卡片），搜索过滤或整组隐藏时用。 */
    public LinearLayout view() { return group; }

    /** 卡片本身，需要放自定义内容时用。 */
    public LinearLayout card() { return card; }

    /** 之后加的行之间画 0.5dp 的分隔线，从左边 `insetDp` 处开始（语言卡那种带图标的行用 58）。 */
    public GroupCard withDividers(int insetDp) {
        dividerInset = insetDp;
        return this;
    }

    /** 导航行：行尾是当前值和 ›，点了进下一页或打开选择面板。`action` 为 null 时整行禁用。 */
    public Row nav(CharSequence title, @Nullable CharSequence subtitle, @Nullable CharSequence value,
            @Nullable Runnable action) {
        Row row = new Row(this, title, subtitle, false);
        row.value = trailingValue(row, value);
        ImageView chevron = Ui.chevron(context);
        LinearLayout.LayoutParams params = Ui.squareParams(context, Ui.CHEVRON_SIZE);
        params.setMarginStart(Ui.dp(context, 6));
        row.view.addView(chevron, params);
        row.setAction(action);
        return add(row);
    }

    /** 值行：只显示，行尾是值，不能点。 */
    public Row value(CharSequence title, @Nullable CharSequence subtitle, @Nullable CharSequence value) {
        Row row = new Row(this, title, subtitle, false);
        row.value = trailingValue(row, value);
        return add(row);
    }

    /** 开关行：点整行或开关都会切换，`onChange` 收到切换后的状态；从代码里改状态用 {@link Row#setChecked}，不会回调。 */
    public Row toggle(CharSequence title, @Nullable CharSequence subtitle, boolean checked,
            Consumer<Boolean> onChange) {
        Row row = new Row(this, title, subtitle, true);
        MsSwitch control = new MsSwitch(context);
        control.setChecked(checked);
        ViewPolicy.setInteractive(control, false);
        Ui.hideFromAccessibility(control);
        LinearLayout.LayoutParams params = Ui.rowGapParams(context);
        row.view.addView(control, params);
        row.toggle = control;
        ViewPolicy.bindClick(row.view, () -> {
            control.toggle();
            onChange.accept(control.isChecked());
        });
        ViewPolicy.setInteractive(row.view, true);
        ViewPolicy.setBackground(row.view, Ui.ripple(context));
        row.view.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Switch.class.getName());
                info.setCheckable(true);
                info.setChecked(control.isChecked());
            }
        });
        return add(row);
    }

    /**
     * 滑块行：标题、110dp 的滑块和右侧 46dp 宽的数值标签。
     *
     * @param label 把值格式化成标签文字（例如 `v -> v + "%"`）
     * @param onCommit 一次修改完成时回调，见 {@link MsSlider#setOnValueCommit}
     */
    public Row slider(CharSequence title, int min, int max, int step, int value,
            IntFunction<CharSequence> label, IntConsumer onCommit) {
        Row row = new Row(this, title, null, false);
        MsSlider control = new MsSlider(context);
        control.setRange(min, max, step);
        control.setValue(value);
        control.setContentDescription(title);
        LinearLayout.LayoutParams sliderParams = new LinearLayout.LayoutParams(
            Ui.dp(context, Ui.SLIDER_WIDTH), Ui.dp(context, Ui.SLIDER_TOUCH_HEIGHT));
        sliderParams.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
        row.view.addView(control, sliderParams);

        TextView text = Ui.styledLabel(context, label.apply(control.value()), 13, 400, Ui.subText(context));
        ViewPolicy.setEndCenteredVertically(text);
        ViewPolicy.setSingleLine(text);
            Ui.hideFromAccessibility(text);
        row.view.addView(text, new LinearLayout.LayoutParams(Ui.dp(context, Ui.SLIDER_LABEL_WIDTH),
            ViewGroup.LayoutParams.WRAP_CONTENT));
        control.setOnValueChange(current -> {
            CharSequence shown = label.apply(current);
            text.setText(shown);
            ViewCompat.setStateDescription(control, shown);
        });
        ViewCompat.setStateDescription(control, label.apply(control.value()));
        control.setOnValueCommit(onCommit);
        row.slider = control;
        row.value = text;
        return add(row);
    }

    /** 按钮行：行尾一个 tonal 胶囊按钮（accentSoft 底、强调色字，13sp）。 */
    public Row button(CharSequence title, @Nullable CharSequence subtitle, CharSequence label, Runnable action) {
        Row row = new Row(this, title, subtitle, false);
        TextView button = KeyboardSheets.tonalButton(context, label, label + "，" + title, 500, action);
        LinearLayout.LayoutParams params = Ui.rowGapParams(context);
        row.view.addView(button, params);
        row.button = button;
        return add(row);
    }

    /** 卡片里的一段说明文字，14sp 次要文字色。 */
    public TextView note(CharSequence text) {
        TextView note = Ui.styledLabel(context, text, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
        Ui.setSymmetricPaddingDp(note, context, Ui.ROW_PADDING_H, 12);
        addDivider();
        card.addView(note, Ui.matchWidth());
        return note;
    }

    /** 卡片下方的脚注，13sp 次要文字色，与组标题同样缩进。 */
    public TextView footer(CharSequence text) {
        TextView note = Ui.styledLabel(context, text, 13, 400, Ui.subText(context));
        Ui.setPaddingDp(note, context, Ui.GROUP_TITLE_INSET, 8,
            Ui.GROUP_TITLE_INSET, 0);
        group.addView(note, Ui.matchWidth());
        return note;
    }

    /** 放一个自定义的行（例如语言卡的行、词条行），按需要在它前面画分隔线。 */
    public <T extends View> T addView(T row) {
        addDivider();
        card.addView(row, Ui.matchWidth());
        return row;
    }

    private Row add(Row row) {
        addDivider();
        card.addView(row.view, Ui.matchWidth());
        return row;
    }

    private void addDivider() {
        if (dividerInset < 0 || card.getChildCount() == 0) return;
        View rule = Ui.divider(context, true);
        LinearLayout.LayoutParams params = (LinearLayout.LayoutParams) rule.getLayoutParams();
        params.setMarginStart(Ui.dp(context, dividerInset));
        card.addView(rule, params);
    }

    private TextView trailingValue(Row row, @Nullable CharSequence value) {
        TextView text = Ui.trailingValue(context, "", Ui.TEXT_ROW_TITLE, Ui.subText(context));
        LinearLayout.LayoutParams params = Ui.rowGapParams(context);
        row.view.addView(text, params);
        setText(text, value);
        return text;
    }

    private static void setText(TextView view, @Nullable CharSequence text) {
        view.setText(text);
        ViewPolicy.setVisibilityForText(view, text);
    }

    /** 一行设置；保留各部件的引用，页面在数据变化后原地改写它们。 */
    public static final class Row {
        private final LinearLayout view;
        private final TextView title;
        private final TextView subtitle;
        @Nullable private TextView value;
        @Nullable private MsSwitch toggle;
        @Nullable private MsSlider slider;
        @Nullable private TextView button;

        private Row(GroupCard owner, CharSequence titleText, @Nullable CharSequence subtitleText, boolean interceptTouches) {
            Context context = owner.context;
            // 开关行整行接收点按：开关本身不单独响应拖动，否则拖动开关和点整行会各切换一次。
            view = interceptTouches ? new LinearLayout(context) {
                @Override public boolean onInterceptTouchEvent(MotionEvent event) { return true; }
            } : new LinearLayout(context);
            view.setOrientation(LinearLayout.HORIZONTAL);
            ViewPolicy.setCenteredVertically(view);
            Ui.setRowMinimumHeight(view, owner.context);
            Ui.setRowPadding(view, owner.context);

            LinearLayout texts = Ui.column(context);
            title = Ui.styledLabel(context, titleText, Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
            texts.addView(title);
            subtitle = Ui.styledLabel(context, subtitleText, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
            LinearLayout.LayoutParams subtitleParams = Ui.wrap();
            subtitleParams.topMargin = Ui.dp(owner.context, 1);
            texts.addView(subtitle, subtitleParams);
            setText(subtitle, subtitleText);
            view.addView(texts, Ui.weightWrap(1f));
        }

        private void setAction(@Nullable Runnable action) {
            ViewPolicy.setBackground(view, action == null ? null : Ui.ripple(view.getContext()));
            ViewPolicy.setInteractive(view, action != null);
            ViewPolicy.bindOptionalClick(view, action);
            ViewPolicy.setEnabledWithAlpha(view, action != null, 0.38f);
        }

        /** 整行。 */
        public View view() { return view; }

        public void setTitle(CharSequence text) { title.setText(text); }

        public void setSubtitle(@Nullable CharSequence text) { setText(subtitle, text); }

        /** 改写行尾的值（导航行、值行、滑块行的数值标签）。 */
        public void setValue(@Nullable CharSequence text) {
            if (value != null) setText(value, text);
        }

        /** 开关行从代码里改状态，不触发回调。 */
        public void setChecked(boolean checked) {
            if (toggle != null) toggle.setChecked(checked);
        }

        /** 滑块行从代码里改值，标签随之刷新，不触发保存。 */
        public void setSliderValue(int current) {
            if (slider != null) slider.setValue(current);
        }

        @Nullable public MsSwitch toggle() { return toggle; }

        @Nullable public MsSlider slider() { return slider; }

        @Nullable public TextView button() { return button; }

        /** 禁用的行仍然显示，只是变淡且不响应。 */
        public void setEnabled(boolean enabled) {
            ViewPolicy.setEnabledWithAlpha(view, enabled, 0.38f);
            if (toggle != null) ViewPolicy.setEnabled(toggle, enabled);
            if (slider != null) ViewPolicy.setEnabled(slider, enabled);
            if (button != null) ViewPolicy.setEnabled(button, enabled);
        }

        public void setVisible(boolean visible) {
            ViewPolicy.setVisible(view, visible);
        }
    }
}
