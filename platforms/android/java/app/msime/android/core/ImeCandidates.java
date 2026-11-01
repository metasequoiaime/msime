package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.graphics.Typeface;
import android.os.Build;
import android.util.TypedValue;
import android.view.Menu;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONArray;
import org.json.JSONObject;

/** 候选按钮的构建与样式、展开的候选面板和候选长按菜单；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImeCandidates {
    private final MSIMEInputService s;
    private String styleCacheKey;
    private int selectedBackground;
    private int selectedText;
    private int keyBackground;
    private int keyForeground;
    private int accentSoft;
    private int accentText;
    private Typeface candidateTypeface;
    private CandidateAppearance.Layout candidateLayout;
    /** 上次解析样式缓存时的外观对象；外观对象不可变，同一个对象就不必再拼一遍 key()。 */
    private CandidateAppearance.Palette styledAppearance;
    /** 候选条 chip 与展开网格单元共用的文字配色（选中文字、普通文字）；ColorStateList 不可变，各按钮共用一份。 */
    private android.content.res.ColorStateList candidateTextColors;
    /**
     * 每个候选按钮上次装上的底。外观和密度都没变、按钮的底也没被别处换掉时，样式通道不再给它新建五块 GradientDrawable 和一个 StateListDrawable：render 末尾的换肤每次都会走到每个候选，低端机上每键几百个短命 Drawable。外观或密度一变整个作废。
     */
    private final java.util.Map<Button, android.graphics.drawable.Drawable> chipFaces =
        new java.util.WeakHashMap<>();
    private float chipDensity;
    /** 整面展开网格；滚动视图随键盘视图重建时跟着换。 */
    private ImeCandidateGrid expandedGrid;

    ImeCandidates(MSIMEInputService s) {
        this.s = s;
    }

    /** 每次外观变化只解析一次；候选包生效时，候选按钮与候选面板使用同一套配色。 */
    private void ensureStyleCache() {
        float density = DimensionPolicy.density(s);
        if (density != chipDensity) {
            chipDensity = density;
            chipFaces.clear();
        }
        if (s.candidateAppearance == styledAppearance) return;
        styledAppearance = s.candidateAppearance;
        String key = s.candidateAppearance.key();
        if (key.equals(styleCacheKey)) return;
        styleCacheKey = key;
        chipFaces.clear();
        selectedBackground = s.candidateAppearance.chip();
        selectedText = s.candidateAppearance.chipText();
        keyBackground = s.candidateAppearance.canvas() == null
            ? s.candidateAppearance.surface() : android.graphics.Color.TRANSPARENT;
        keyForeground = s.candidateAppearance.text();
        accentSoft = selectedBackground;
        accentText = selectedText;
        candidateTypeface = s.imeStyler.candidateTypeface();
        candidateLayout = s.candidateAppearance.layout();
        // 展开网格单元的 accentText 与 chip 的 selectedText 是同一个色，两处共用这一份。
        candidateTextColors = ColorPolicy.stateList(
            new int[][] {{android.R.attr.state_selected}, {}},
            new int[] {selectedText, keyForeground});
    }

    /** 按钮现在的底就是上次在当前外观和密度下装上的那块。 */
    private boolean keepsChipFace(Button button) {
        android.graphics.drawable.Drawable background = button.getBackground();
        return background != null && chipFaces.get(button) == background;
    }

    /** 首选候选 chip：字母键的底（kb.key）、圆角 9，皮肤强调色 600 字重；其余候选不画底。 */
    private android.graphics.drawable.GradientDrawable chip(int color, int radiusDp) {
        CandidateCanvasAssets canvas = s.candidateAppearance.canvas();
        CandidateAppearance.Layout layout = candidateLayout;
        float radius = canvas == null || canvas.itemRadius == null
            ? s.pixels(layout == null ? radiusDp : (int) Math.round(layout.itemRadius))
            : (float) (canvas.itemRadius * s.getResources().getDisplayMetrics().density);
        return DrawablePolicy.rounded(color, radius);
    }

    void styleCandidateButton(Button button) {
        ensureStyleCache();
        if (expandedCells.contains(button)) {
            styleExpandedCell(button);
            return;
        }
        if (!keepsChipFace(button)) {
            android.graphics.drawable.Drawable pressed =
                s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
            android.graphics.drawable.Drawable focused =
                s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
            android.graphics.drawable.Drawable hovered =
                s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
            android.graphics.drawable.StateListDrawable states = DrawablePolicy.stateList(
                new int[][] {
                    {android.R.attr.state_selected}, {android.R.attr.state_pressed},
                    {android.R.attr.state_focused}, {android.R.attr.state_hovered}, new int[0]
                },
                chip(selectedBackground, 9), pressed, focused, hovered,
                chip(android.graphics.Color.TRANSPARENT, 9));
            ViewPolicy.setBackground(button, states);
            chipFaces.put(button, states);
        }
        ViewPolicy.setTextColor(button, candidateTextColors);
        applyCandidateTypeface(button);
        // 候选条只有触控下限，不取皮肤的 itemMinWidth（见 CandidateAppearance.STRIP_CHIP_MIN_WIDTH_DP）。
        ViewPolicy.setMinimumWidth(button, s.pixels(CandidateAppearance.STRIP_CHIP_MIN_WIDTH_DP));
        ViewPolicy.clearMinimumHeight(button);
        int horizontal = candidateLayout == null || !Double.isFinite(candidateLayout.itemPaddingRight)
            ? 11 : (int) Math.ceil(candidateLayout.itemPaddingRight);
        LayoutPolicy.setHorizontalPaddingDp(button, s, Math.max(0, horizontal));
        int vertical = candidateLayout == null || !Double.isFinite(candidateLayout.itemExtraHeight) ? 0
            : s.pixels((int) Math.ceil(Math.min(4, candidateLayout.itemExtraHeight / 2)));
        button.setPadding(button.getPaddingLeft(), vertical, button.getPaddingRight(), vertical);
        if (button.getLayoutParams() instanceof LinearLayout.LayoutParams params) {
            double gap = candidateLayout == null || !Double.isFinite(candidateLayout.itemGap) ? 0
                : candidateLayout.itemGap + (candidateLayout.fitContent ? 0 : 6);
            int margin = s.pixels((int) Math.round(gap));
            // setLayoutParams 每次都会 requestLayout；边距没变就不再设。
            if (params.rightMargin != margin) {
                params.rightMargin = margin;
                button.setLayoutParams(params);
            }
        }
        ViewPolicy.setLineSpacing(button, 0, 1.0f);
        ViewPolicy.clearFontPadding(button);
        ViewPolicy.clearElevation(button);
    }

    private void applyCandidateTypeface(Button button) {
        ViewPolicy.setTypeface(button, candidateTypeface,
            button.isSelected() ? Typeface.BOLD : Typeface.NORMAL);
    }

    /** 展开网格里的单元：样式通道重走整棵树时按网格单元上色，而不是按候选条的 chip。 */
    private final java.util.Set<Button> expandedCells =
        java.util.Collections.newSetFromMap(new java.util.WeakHashMap<>());

    /** 展开网格单元格：圆角 8，底色和选中色取候选栏配色；高度和文字大小跟随候选字号与候选栏大小，见 {@link CandidateScalePolicy#expandedCellHeight}。 */
    private void styleExpandedCell(Button button) {
        if (!keepsChipFace(button)) {
            android.graphics.drawable.StateListDrawable states = DrawablePolicy.stateList(
                new int[][] {
                    {android.R.attr.state_selected}, {android.R.attr.state_pressed}, new int[0]
                },
                chip(accentSoft, 8), chip(s.candidateAppearance.hover(), 8), chip(keyBackground, 8));
            ViewPolicy.setBackground(button, states);
            chipFaces.put(button, states);
        }
        ViewPolicy.setTextColor(button, candidateTextColors);
        applyCandidateTypeface(button);
        ViewPolicy.clearElevation(button);
    }

    boolean showCandidateMenu(Button button, int slot, JSONObject id, String text) {
        if (!s.candidateGlossInsertionEnabled() && !s.candidateManagementEnabled()) return false;
        return showCandidateMenu(button, slot, id, text, s.visibleCandidate(slot), false);
    }

    boolean showCandidateMenu(Button button, int slot, JSONObject id, String text,
                                      JSONObject candidate, boolean expanded) {
        return showCandidateMenu(button, slot, id, text, candidate, expanded, null);
    }

    boolean showCandidateMenu(Button button, int slot, JSONObject id, String text,
                              JSONObject candidate, boolean expanded, JSONObject stripSnapshot) {
        if (!s.candidateGlossInsertionEnabled() && !s.candidateManagementEnabled()) return false;
        PopupMenu popup = new PopupMenu(s, button);
        String translation = candidate == null || candidate.isNull("translation")
            || !CandidateTranslationPolicy.userTranslationSourceAllowed(
                InputViewValuePolicy.integer(candidate, "source", -1))
            ? "" : JsonPolicy.strictStringOrEmpty(candidate.opt("translation"));
        java.util.List<String> glosses = s.candidateGlossInsertionEnabled()
            ? CandidateTranslationPolicy.insertionGlosses(translation) : java.util.List.of();
        for (int index = 0; index < glosses.size(); index++)
            popup.getMenu().add(Menu.NONE, 2000 + index, Menu.NONE, glosses.get(index));
        boolean management = s.candidateManagementEnabled();
        if (glosses.isEmpty() && !management) return false;
        if (management) {
            for (CandidateManagementAction action : CandidateManagementAction.values()) {
                if (action.confirmationRequired()) {
                    // 删除收进子菜单，和 iOS 的嵌套 UIMenu 一样：点开「删除词条…」当场展开「确认删除此词条」，
                    // 一次长按就能删完，不用退出菜单再长按一次，也不弹对话框（#5605、#5653）。
                    popup.getMenu().addSubMenu(Menu.NONE, Menu.NONE, action.ordinal(), action.title())
                        .add(Menu.NONE, action.menuItemId(), Menu.NONE, action.confirmationTitle());
                    continue;
                }
                popup.getMenu().add(Menu.NONE, action.menuItemId(), action.ordinal(), action.title());
            }
        }
        // 「加入生词本」排在最后，和 iOS、鸿蒙的长按菜单、桌面的候选右键菜单同一个位置；只在这一行显示着可上屏的释义时出现。
        boolean personalWordbook = CandidateManagementAction.offersPersonalWordbook(glosses);
        // 「朗读」紧挨在它前面：有能读的英文（显示着的英文释义，或候选本身是英文）、本机又装着语音引擎时出现。
        String readAloud = s.readAloudText(text, translation);
        if (readAloud != null)
            popup.getMenu().add(Menu.NONE, CandidateManagementAction.READ_ALOUD_MENU_ITEM,
                CandidateManagementAction.READ_ALOUD_ORDER, CandidateManagementAction.READ_ALOUD_TITLE);
        if (personalWordbook)
            popup.getMenu().add(Menu.NONE, CandidateManagementAction.PERSONAL_WORDBOOK_MENU_ITEM,
                CandidateManagementAction.PERSONAL_WORDBOOK_ORDER, CandidateManagementAction.PERSONAL_WORDBOOK_TITLE);
        popup.setOnMenuItemClickListener(item -> {
            s.imeKeyFeedback.playFeedback(button);
            if (readAloud != null && item.getItemId() == CandidateManagementAction.READ_ALOUD_MENU_ITEM) {
                s.readAloud(readAloud);
                return true;
            }
            if (personalWordbook && item.getItemId() == CandidateManagementAction.PERSONAL_WORDBOOK_MENU_ITEM) {
                s.addCandidateToPersonalWordbook(text, translation,
                    candidate == null ? "" : s.candidateDisplayedReading(candidate));
                return true;
            }
            int glossIndex = item.getItemId() - 2000;
            if (glossIndex >= 0 && glossIndex < glosses.size()) {
                if (stripSnapshot != null)
                    s.insertStripCandidateGloss(stripSnapshot, candidate, id, text, glosses.get(glossIndex));
                else if (expanded) s.insertExpandedCandidateGloss(candidate, id, text, glosses.get(glossIndex));
                else s.insertCandidateGloss(slot, id, text, glosses.get(glossIndex));
                return true;
            }
            CandidateManagementAction action;
            try {
                action = CandidateManagementAction.fromMenuItemId(item.getItemId());
            } catch (IllegalArgumentException error) {
                return false;
            }
            s.editCandidate(id, action);
            return true;
        });
        popup.show();
        return true;
    }

    Button makeCandidateButton(int slot) {
        // Apple uses the same press-feedback button for candidate chips as for keys. Android's
        // HorizontalScrollView cancels the child on a drag, so the button keeps immediate tap
        // feedback without changing the existing scroll-versus-select boundary.
        KeyboardPressButton button = candidateButton();
        ViewPolicy.bindClick(button, () -> s.selectVisibleCandidate(button, slot));
        s.bindCandidateRenderRelease(button);
        button.setOnLongClickListener(ignored -> {
            JSONObject current = s.visibleCandidate(slot);
            JSONObject id = current == null ? null : current.optJSONObject("id");
            if (id == null || (!s.candidateManagementEnabled() && !s.candidateGlossInsertionEnabled())) return false;
            // AI 占位行（来源 12，共享内核已经不再产出）没有可管理的词，长按不弹菜单（#6888）；平时它也不可长按，见 updateCandidateButton。
            if (!AiPendingCandidatePolicy.offersMenu(InputViewValuePolicy.integer(current, "source", -1))) return false;
            String text = s.chineseCandidateOutput(InputViewValuePolicy.textOr(current, "text", ""), s.view,
                InputViewValuePolicy.integer(current, "source", -1));
            return showCandidateMenu(button, slot, id, text);
        });
        return button;
    }

    /** 横向完整候选条的一个按钮；由 {@link ImeCandidateStrip} 按位置复用，每次 render 经 {@link #bindStripCandidateButton} 换上当前候选。 */
    KeyboardPressButton newStripCandidateButton() {
        KeyboardPressButton button = candidateButton();
        s.bindCandidateRenderRelease(button);
        return button;
    }

    /** 横向完整候选按全局身份选择，点击不再依赖引擎当前页的槽位。按钮会复用：点击和长按每次都重新绑到这一代的候选与快照上。 */
    void bindStripCandidateButton(KeyboardPressButton button, JSONObject snapshot, JSONObject candidate,
                                  int visibleSlot) {
        JSONObject id = candidate.optJSONObject("id");
        ViewPolicy.bindClick(button, () -> s.selectStripCandidate(button, candidate));
        if (AiPendingCandidatePolicy.offersMenu(InputViewValuePolicy.integer(candidate, "source", -1)))
            button.setOnLongClickListener(ignored -> {
                if (!s.candidateVersionIsCurrent(id)) return false;
                String text = s.chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), s.view,
                    InputViewValuePolicy.integer(candidate, "source", -1));
                return showCandidateMenu(button, -1, id, text, candidate, false, snapshot);
            });
        else button.setOnLongClickListener(null);
        // 放在绑长按之后：setOnLongClickListener 总会把按钮设成可长按，updateCandidateButton 再按来源改回去。
        s.updateCandidateButton(button, candidate, visibleSlot, visibleSlot);
    }

    Button expandedCandidateButton(JSONObject candidate) {
        JSONObject id = candidate.optJSONObject("id");
        KeyboardPressButton button = candidateButton();
        s.bindCandidateRenderRelease(button);
        int source = InputViewValuePolicy.integer(candidate, "source", -1);
        String text = s.chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), s.view,
            InputViewValuePolicy.integer(candidate, "source", -1));
        ((ContactCandidateButton) button).setContactSource(source);
        boolean highlighted = InputViewValuePolicy.booleanValue(candidate, "highlighted", false);
        String typed = s.candidatePanelSnapshot == null ? ""
            : InputViewValuePolicy.textOr(s.candidatePanelSnapshot, "preedit", "");
        String annotation = s.candidatePronunciationAnnotation(s.candidateAnnotation(candidate, typed), candidate);
        boolean corrected = CandidateCorrectionMarkPolicy.confident(candidate.opt(CandidateCorrectionMarkPolicy.FIELD));
        button.setText(s.candidateLabel("", text, annotation, highlighted, corrected));
        int labelLines = MSIMEInputService.candidateLabelLines(annotation);
        ViewPolicy.setFixedLines(button, labelLines);
        s.configureCandidateTextLayout(button, labelLines);
        if (s.candidatePronunciationVisible())
            button.setEllipsize(android.text.TextUtils.TruncateAt.END);
        KeyboardTypographyPolicy.setKeyTextSize(button, CandidateScalePolicy.expandedTextSize(s.candidateFontSize));
        ViewPolicy.setSelected(button, highlighted);
        // AI 占位行画淡，云占位行由 ImeCandidateGrid 直接隐藏，免得内部标记进入手机候选面板（#6888）。
        boolean placeholder = AiPendingCandidatePolicy.isPlaceholder(source)
            || AiPendingCandidatePolicy.isCloudPending(source);
        button.setAlpha(AiPendingCandidatePolicy.alpha(source));
        expandedCells.add(button);
        // 格子宽度由网格的列决定（CandidateGridPolicy），这里只量自然宽度。Button 默认样式自带 88 dp 的 minWidth，View 级的 setMinimumWidth 盖不掉它：每格至少 88 dp，手机上一行只放得下三格，右边空出一截。
        ViewPolicy.clearMinimumWidth(button);
        ViewPolicy.setMinimumHeight(button, s.pixels(CandidateScalePolicy.expandedCellHeight(
            CandidateScalePolicy.EXPANDED_CELL_HEIGHT_DP, s.candidateFontSize, s.candidateScalePercent)));
        LayoutPolicy.setHorizontalPaddingDp(button, s, 10);
        // The completed keyboard tree is styled once by MSIMEInputService.render().
        // Styling here would be repeated immediately after this button is attached.
        long index = id == null ? -1
            : CandidateGlossPolicy.strictOr(id.opt("index"), -1);
        button.setContentDescription(index < 0 ? "候选" : AiPendingCandidatePolicy.description(source,
            "候选 " + (index + 1) + "：", ContactCandidatePolicy.description(source, text),
            s.candidateAccessibilitySuffix(candidate, typed)
                + s.candidatePronunciationAccessibilitySuffix(candidate)
                + CandidateCorrectionMarkPolicy.accessibilitySuffix(corrected)));
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        if (id == null || index < 0) {
            ViewPolicy.setEnabled(button, false);
        } else {
            ViewPolicy.bindClick(button, () -> {
                s.imeKeyFeedback.playFeedback(button);
                // 点占位行什么也不发生：Engine 本来就不上屏，这里连面板也不收起，用户还在看完整列表（#6888）。
                if (placeholder || !s.candidateVersionIsCurrent(id)) return;
                s.selectPanelCandidate(id);
            });
            // 占位行不挂长按：没有可管理的词，读屏也不该提示长按（#6888）。
            if (AiPendingCandidatePolicy.offersMenu(source)) button.setOnLongClickListener(ignored -> {
                if (!s.candidateManagementEnabled() && !s.candidateGlossInsertionEnabled()) return false;
                return showCandidateMenu(button, -1, id, text, candidate, true);
            });
        }
        return button;
    }

    private KeyboardPressButton expandedActionButton(String label, float sizeSp,
            String description, Runnable action) {
        KeyboardPressButton button = candidateButton();
        s.bindCandidateRenderRelease(button);
        button.setKeyboardRole(KeyboardKeyRole.ACCENT);
        ViewPolicy.setTextSizeLabel(button, label, sizeSp);
        KeyboardTypographyPolicy.setKeyTextSize(button, sizeSp);
        button.setContentDescription(description);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
        return button;
    }

    private KeyboardPressButton candidateButton() {
        return new ContactCandidateButton(s);
    }

    void renderExpandedCandidates() {
        if (s.expandedCandidates == null || s.expandedCandidateScroll == null) return;
        if (!s.candidatePanelOpen) {
            // The normal keyboard render reaches this method on every keystroke. The panel is
            // normally already hidden by closeCandidatePanel(); avoid traversing and clearing an
            // empty subtree until the next open actually needs to rebuild it.
            if (!ViewPolicy.isGone(s.expandedCandidates)) {
                ViewPolicy.hide(s.expandedCandidates);
                ViewPolicy.hide(s.expandedCandidateScroll);
            }
            // 候选条选词等路径只把 candidatePanelOpen 置为 false，九键三栏面板在这里跟着收起并清掉筛选；面板里部分选词后留着面板的路径见 selectPanelCandidate。
            s.imeNineKeyPanel.dismiss();
            return;
        }
        // 全拼九键的三栏面板：选拼音、⌫、筛选换了一代候选时不收起，重新取完整候选再画；组字结束时 eligible 为假，落到下面按代次收起。普通网格按代次收起，部分选词后由 selectPanelCandidate 先换上新一代的快照再画，走不到这里的收起。
        if (s.imeNineKeyPanel.eligible() && s.imeNineKeyPanel.refreshSnapshot()) {
            if (!ViewPolicy.isGone(s.expandedCandidates)) {
                s.expandedCandidates.removeAllViews();
                ViewPolicy.hide(s.expandedCandidates);
                ViewPolicy.hide(s.expandedCandidateScroll);
            }
            s.imeNineKeyPanel.render();
            return;
        }
        s.imeNineKeyPanel.dismiss();
        s.expandedCandidates.removeAllViews();
        if (s.view == null || s.candidatePanelSnapshot == null
                || CandidateGlossPolicy.strictOr(s.candidatePanelSnapshot.opt("session"), Long.MIN_VALUE)
                    != s.session
                || !MSIMEInputService.sameCandidateVersion(s.candidatePanelSnapshot, s.view)) {
            s.closeCandidatePanel();
            return;
        }
        ViewPolicy.show(s.expandedCandidateScroll);
        ViewPolicy.show(s.expandedCandidates);
        LayoutPolicy.setSymmetricPaddingDp(s.expandedCandidates, s, 8, 6);
        JSONArray entries = s.candidatePanelSnapshot.optJSONArray("candidates");
        int count = entries == null ? 0 : entries.length();
        String reading = InputViewValuePolicy.textOr(s.candidatePanelSnapshot, "reading", "");
        String compositionText = reading.isEmpty()
            ? InputViewValuePolicy.textOr(s.candidatePanelSnapshot, "preedit", "") : reading;
        // 设计的网格不画标题；组合文本和候选总数留在网格的描述里给读屏。格子分批建，滚到接近底部再追加（#6471）。
        if (expandedGrid == null || !expandedGrid.drives(s.expandedCandidateScroll))
            expandedGrid = new ImeCandidateGrid(s, s.expandedCandidateScroll, true);
        CandidateWrapLayout list = expandedGrid.build(s.candidatePanelSnapshot, entries,
            "完整候选列表；" + compositionText + "；" + count + " 个候选");
        s.expandedCandidates.addView(list, LayoutPolicy.weightedWidthParams(1));
        // 底部 返回 + ⌫，各 40 dp 高、功能键底色。
        LinearLayout footer = LayoutPolicy.row(s);
        KeyboardPressButton close = expandedActionButton("返回", 15, "收起候选面板", () -> {
            s.closeCandidatePanel();
            s.render();
        });
        KeyboardPressButton delete = expandedActionButton("⌫", 16, "候选面板 删除", () -> {
            s.closeCandidatePanel();
            s.deleteFromHandwriting();
        });
        LinearLayout.LayoutParams closeParams = LayoutPolicy.weightedHeightParams(s, 40, 1);
        LinearLayout.LayoutParams deleteParams = LayoutPolicy.weightedHeightParams(s, 40, 1);
        deleteParams.setMarginStart(s.pixels(6));
        footer.addView(close, closeParams);
        footer.addView(delete, deleteParams);
        LinearLayout.LayoutParams footerParams = LayoutPolicy.matchWidthWrapParams();
        footerParams.topMargin = s.pixels(6);
        s.expandedCandidates.addView(footer, footerParams);
    }
}
