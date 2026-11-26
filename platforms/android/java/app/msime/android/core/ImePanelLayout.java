package app.msime.android;

import android.graphics.Paint;
import android.graphics.Typeface;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 面板高度的唯一写入处：顶栏各行、键区各排和键盘列的底部留白都在这里按 {@link PanelGeometry} 算出的值写进视图。
 *
 * <p>这里只收集输入（量字形、读设置和视图此刻显不显示）和写视图，算法都在 {@link PanelGeometry}。读音行的字形只在字号、字体或内边距变了时才重量，打字时每次刷新只比较几个数。写视图经 {@link ViewPolicy#setFixedHeight} 和 {@link ViewPolicy#setPaddingIfChanged}，值没变就不触发重新布局。
 */
final class ImePanelLayout {
    private final MSIMEInputService s;
    /** 上一次量读音行时各个标签（读音和读音行里的文字子视图）的字号、字体和上下内边距，以及当时的设计高度、读音行内边距和量出的高度。 */
    private float[] readingSizes = new float[0];
    private Typeface[] readingFaces = new Typeface[0];
    private int[] readingPaddings = new int[0];
    private int readingDesign = -1;
    private int readingHeaderPadding = -1;
    private int readingHeight;

    ImePanelLayout(MSIMEInputService s) {
        this.s = s;
    }

    /**
     * 顶栏：读音行、候选行、工具栏那一行（工具栏、最近复制、自动填充共用这一高度）和两种状态之间的补齐空白。每次 render 末尾调用。
     *
     * @param reservedRows 候选行要为释义和提示多留的行数（{@link CandidateTranslationPolicy#reservedRows}）
     * @param readingRowReserved 读音行要预留（{@link SpellingEditorPolicy#reserveHeader}）
     * @param inlineAutofill 自动填充那一行；API 30 以下为 null
     */
    void applyTopBand(int reservedRows, boolean readingRowReserved, ImeInlineAutofill inlineAutofill) {
        if (s.candidateLine == null) return;
        boolean customSize = s.localSettings != null && s.localSettings.has(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT);
        int sizePercent = customSize ? s.localSettings.integer(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT) : 100;
        boolean idleRowVisible = ViewPolicy.isVisible(s.shortcutToolbar)
            || ViewPolicy.isVisible(s.recentClipRow)
            || inlineAutofill != null && inlineAutofill.visible();
        PanelGeometry.TopBand band = PanelGeometry.topBand(new PanelGeometry.TopBandInputs(
            readingRowPx(readingRowReserved), ViewPolicy.isVisible(s.candidateHeader),
            s.pixels(s.imeCandidates.stripHeightDp(reservedRows)),
            // 同一状态下「候选栏大小」为 100% 时的设计行高：读音行可见时只取候选行，按钮和图标不因标题出现而改变比例。
            s.pixels(CandidateStripHeightPolicy.designHeightDp(reservedRows)),
            customSize ? s.pixels(ToolbarSizePolicy.heightDp(sizePercent)) : 0,
            idleRowVisible, ViewPolicy.isVisible(s.candidateLine)));
        ViewPolicy.setFixedHeight(s.candidateLine, band.candidateLine());
        if (s.candidateHeader != null) ViewPolicy.setFixedHeight(s.candidateHeader, band.readingRow());
        if (s.shortcutToolbar != null) ViewPolicy.setFixedHeight(s.shortcutToolbar, band.idleRow());
        if (inlineAutofill != null) inlineAutofill.setHeight(band.idleRow());
        // 「最近复制」占的是工具栏那一行的位置，同高，出现和消失时键盘不跳。
        if (s.recentClipRow != null) ViewPolicy.setFixedHeight(s.recentClipRow, band.idleRow());
        if (s.candidateHeightSpacer != null) ViewPolicy.setFixedHeight(s.candidateHeightSpacer, band.spacer());
        s.imeToolbar.applyShortcutSize(sizePercent, customSize, band.idleRow(), band.designRow());
    }

    /**
     * 读音行至少是设计的 14 dp，读音字号放不下时按读音文字的实际高度加上和候选行之间的间距加高（{@link ReadingRowPolicy}）。「清除」等按钮的字号和字体留白可能比读音大，也一起量；收起时仍预留同一行高，避免输入时键盘跳动。
     */
    private int readingRowPx(boolean reserved) {
        if (!reserved) return 0;
        int design = s.pixels(ImeToolbar.READING_ROW_DP);
        TextView preedit = s.preedit;
        if (preedit == null) return design;
        LinearLayout header = s.candidateHeader;
        // 读音行自己的上下内边距（下边就是和候选行之间的 READING_GAP_DP）也算进去，否则字号大时间距被读音挤掉。
        int headerPadding = header == null ? 0 : header.getPaddingTop() + header.getPaddingBottom();
        int labels = 1 + (header == null ? 0 : header.getChildCount());
        if (readingCached(design, headerPadding, labels, preedit, header)) return readingHeight;
        readingSizes = new float[labels];
        readingFaces = new Typeface[labels];
        readingPaddings = new int[labels];
        int height = 0;
        for (int index = 0; index < labels; index++) {
            TextView label = readingLabel(index, preedit, header);
            readingSizes[index] = label == null ? -1 : label.getTextSize();
            readingFaces[index] = label == null ? null : label.getTypeface();
            readingPaddings[index] = label == null ? 0 : padding(label);
            if (label != null) height = Math.max(height, readingLabelHeight(label, design, headerPadding));
        }
        readingDesign = design;
        readingHeaderPadding = headerPadding;
        readingHeight = height;
        return height;
    }

    /** 读音行的各个标签和上次量时一样：字号、字体、是否带字体留白和上下内边距都没变，量出来的高度也不会变。 */
    private boolean readingCached(int design, int headerPadding, int labels, TextView preedit, LinearLayout header) {
        if (design != readingDesign || headerPadding != readingHeaderPadding || labels != readingSizes.length)
            return false;
        for (int index = 0; index < labels; index++) {
            TextView label = readingLabel(index, preedit, header);
            if (label == null) {
                if (readingSizes[index] != -1) return false;
                continue;
            }
            if (label.getTextSize() != readingSizes[index] || label.getTypeface() != readingFaces[index]
                    || padding(label) != readingPaddings[index]) return false;
        }
        return true;
    }

    /** 第 0 个是读音，其后是读音行的各个子视图；不是文字的子视图为 null。 */
    private static TextView readingLabel(int index, TextView preedit, LinearLayout header) {
        if (index == 0) return preedit;
        View child = header.getChildAt(index - 1);
        return child instanceof TextView label ? label : null;
    }

    /** 标签自己的上下内边距；带字体留白时取负数，和不带的区分开。 */
    private static int padding(TextView label) {
        int padding = label.getCompoundPaddingTop() + label.getCompoundPaddingBottom();
        return label.getIncludeFontPadding() ? -1 - padding : padding;
    }

    private static int readingLabelHeight(TextView label, int design, int headerPadding) {
        Paint.FontMetricsInt metrics = label.getPaint().getFontMetricsInt();
        int top = label.getIncludeFontPadding() ? metrics.top : metrics.ascent;
        int bottom = label.getIncludeFontPadding() ? metrics.bottom : metrics.descent;
        return ReadingRowPolicy.heightPx(design, top, bottom,
            label.getCompoundPaddingTop() + label.getCompoundPaddingBottom() + headerPadding);
    }

    /** 键区：按 tag 上的 {@link KeyboardHeightRole} 给整棵树里的每一行、每一块定高。整棵树用同一个当前窗口里实际要画的高度调整。 */
    void applyKeyRows(View node) {
        applyKeyRows(node, KeyboardHeightPolicy.windowHeightAdjustment(
                s.touchKeyboardHeightAdjustment, s.getResources().getConfiguration().screenHeightDp),
            s.imeLayoutRows.nineKeyGridDrawn(), DimensionPolicy.density(s),
            s.halfSpacingPixels(s.touchRowSpacingTenths));
    }

    private static void applyKeyRows(View node, int adjustment, boolean nineKeyGridDrawn, float density,
                                     int halfSpacing) {
        if (node.getTag() instanceof KeyboardHeightRole role && node.getLayoutParams() != null) {
            ViewGroup.LayoutParams params = node.getLayoutParams();
            params.height = PanelGeometry.keyRowPx(role, adjustment, nineKeyGridDrawn, density, halfSpacing);
            node.setLayoutParams(params);
        }
        if (node instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++)
                applyKeyRows(group.getChildAt(index), adjustment, nineKeyGridDrawn, density, halfSpacing);
        }
    }

    /**
     * 键盘列的底部留白和键盘底栏的底边距。左右和上边仍是系统栏的 inset；浮动时四边都不留。
     *
     * @param keyboard 键盘的竖向一列
     * @param bar 键盘底栏；没建时为 null
     * @param barShown 底栏这次画着
     * @param inset {@link PanelGeometry#bottomInset} 算出的值
     * @param left 左边的系统栏 inset
     * @param top 上边的系统栏 inset
     * @param right 右边的系统栏 inset
     */
    void applyBottomInset(LinearLayout keyboard, View bar, boolean barShown, PanelGeometry.BottomInset inset,
                          int left, int top, int right) {
        if (barShown && bar != null && bar.getLayoutParams() instanceof FrameLayout.LayoutParams params
                && params.bottomMargin != inset.barMargin()) {
            params.bottomMargin = inset.barMargin();
            bar.setLayoutParams(params);
        }
        ViewPolicy.setPaddingIfChanged(keyboard, left, top, right, inset.padding());
    }
}
