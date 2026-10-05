package app.msime.android;

import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.InsetDrawable;
import android.os.Build;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.MenuItem;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import org.json.JSONArray;
import org.json.JSONObject;

/** 盖在键盘上的各个面板：表情、符号、皮肤、输入方案、剪贴板、回复键盘、AI 润色、本地输入菜单与更多工具的入口；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImePanels {
    private final MSIMEInputService s;

    ImePanels(MSIMEInputService s) {
        this.s = s;
    }

    void showLocalInputMenu() {
        if (s.preedit == null || !s.supportsLocalTools() || s.view == null
                || !s.view.optString("editing_text", "").isEmpty()
                || !"none".equals(s.view.optString("local_mode", "none"))) return;
        PopupMenu popup = new PopupMenu(s, s.preedit);
        for (LocalInputMode mode : s.localInputModes()) {
            MenuItem item = popup.getMenu().add(mode.title());
            item.setEnabled(s.localModeEnabled(mode));
            item.setOnMenuItemClickListener(ignored -> {
                s.openLocalInputMode(mode);
                return true;
            });
        }
        popup.show();
    }

    void renderEmojiStatus() {
        if (s.emojiStatus == null) return;
        // 分类栏只剩图标，分类名改由这一行给出。
        String title = s.emojiSelectedCategory == -1 ? EmojiCatalogModel.RECENTS.title()
            : s.emojiSelectedCategory >= 0 && s.emojiSelectedCategory < EmojiCatalogModel.categories().size()
            ? EmojiCatalogModel.categories().get(s.emojiSelectedCategory).title() : "表情";
        if (s.emojiLoading && s.emojiItems.isEmpty()) s.emojiStatus.setText(title + " · 正在加载…");
        else if (s.emojiItems.isEmpty()) s.emojiStatus.setText(title + " · 暂无表情");
        else s.emojiStatus.setText(title + " · " + s.emojiItems.size() + " 个表情");
    }

    void renderEmojiTabs() {
        if (s.emojiTabs == null) return;
        s.emojiTabs.removeAllViews();
        if (!s.emojiRecents.isEmpty()) addEmojiTab(EmojiCatalogModel.RECENTS, -1);
        for (int index = 0; index < EmojiCatalogModel.categories().size(); index++)
            addEmojiTab(EmojiCatalogModel.categories().get(index), index);
    }

    void addEmojiTab(EmojiCatalogModel.Category entry, int category) {
        KeyboardPressButton tab = new KeyboardPressButton(s);
        tab.setKeyboardRole(KeyboardKeyRole.PLAIN);
        tab.setAllCaps(false);
        tab.setText(entry.icon());
        tab.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
        tab.setPadding(0, 0, 0, 0);
        tab.setMinWidth(0);
        tab.setMinimumWidth(0);
        tab.setMinHeight(0);
        tab.setMinimumHeight(0);
        tab.setSelected(s.emojiSelectedCategory == category);
        tab.setContentDescription("表情分类 " + entry.title());
        if (Build.VERSION.SDK_INT >= 30)
            tab.setStateDescription(tab.isSelected() ? "已选中" : "未选中");
        tab.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(tab);
            s.selectEmojiCategory(category);
        });
        s.emojiTabs.addView(tab, new LinearLayout.LayoutParams(0, s.pixels(40), 1));
    }

    /** 共享换肤遍历之后再画分类栏和状态行：选中的分类是浅强调色圆角底，其余只是半透明图标，不再是一排实心按钮。 */
    void styleEmojiChrome() {
        if (s.emojiTabs != null) {
            for (int index = 0; index < s.emojiTabs.getChildCount(); index++) {
                View tab = s.emojiTabs.getChildAt(index);
                if (tab.isSelected()) {
                    GradientDrawable face = new GradientDrawable();
                    face.setColor(Color.parseColor(s.emojiSkin.accentSoft()));
                    face.setCornerRadius(s.pixels(10));
                    tab.setBackground(new InsetDrawable(face, s.pixels(2), s.pixels(3), s.pixels(2), s.pixels(3)));
                    tab.setAlpha(1f);
                } else {
                    tab.setBackground(null);
                    tab.setAlpha(.5f);
                }
                tab.setElevation(0);
            }
        }
        if (s.emojiStatus != null) s.emojiStatus.setTextColor(ImeStyler.fade(s.emojiSkin.keyForeground(), .55));
    }

    void renderEmojiGrid() {
        if (s.emojiGrid == null) return;
        s.emojiGrid.removeAllViews();
        // 每行固定八等分：不足一行时格子保持原宽，不会被拉满整行。
        LinearLayout row = null;
        for (EmojiCatalogModel.Item item : s.emojiItems) {
            if (row == null || row.getChildCount() == EmojiCatalogModel.COLUMNS) {
                row = new LinearLayout(s);
                row.setWeightSum(EmojiCatalogModel.COLUMNS);
                s.emojiGrid.addView(row, new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(48)));
            }
            Button cell = s.keyboardKey(item.text(), "表情 " + item.text(),
                () -> insertEmoji(item.text()));
            ((KeyboardPressButton) cell).setKeyboardRole(KeyboardKeyRole.PLAIN);
            cell.setTextSize(TypedValue.COMPLEX_UNIT_SP, 28);
            cell.setPadding(0, 0, 0, 0);
            cell.setMinWidth(0);
            cell.setMinimumWidth(0);
            cell.setMinHeight(0);
            cell.setMinimumHeight(0);
            row.addView(cell, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        }
        renderEmojiStatus();
        s.imeStyler.applySkin();
    }

    void insertEmoji(String text) {
        if (!s.emojiPickerVisible() || s.connection == null) return;
        if (!s.commitText(text, TypingSource.LOCAL)) return;
        s.emojiRecents = EmojiCatalogModel.recordRecent(s.emojiRecents, text);
        s.saveEmojiRecents();
    }

    void deleteFromEmojiPicker() {
        if (s.connection != null && !s.command(0))
            s.deleteCodePointBeforeCursor();
    }

    void showEmojiPicker() {
        if (s.session == 0 || s.connection == null || s.emojiPanel == null || s.emojiResources.isEmpty()) {
            Toast.makeText(s, "表情目录尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        s.command(2);
        if (s.session == 0 || s.connection == null) return;
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeMoreTools();
        s.closeSymbolPanel();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        s.emojiRecents = s.loadEmojiRecents();
        s.emojiPanel.setVisibility(View.VISIBLE);
        s.emojiPanel.requestFocus();
        s.selectEmojiCategory(s.emojiRecents.isEmpty() ? 0 : -1);
    }

    void showSymbolPanel() {
        if (s.session == 0 || s.connection == null || s.symbolPanel == null) {
            Toast.makeText(s, "符号面板尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        s.command(2);
        if (s.session == 0 || s.connection == null) return;
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeMoreTools();
        s.closeEmojiPicker();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        s.symbolPanel.resetForPresentation();
        s.symbolPanel.setVisibility(View.VISIBLE);
        s.symbolPanel.requestFocus();
    }

    void showReplyKeyboard() {
        s.replyOpen = true;
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.synchronizeReplyKeyboard();
        s.render();
    }

    void pasteReplySource() {
        try {
            ClipboardManager manager = s.getSystemService(ClipboardManager.class);
            if (manager == null || !manager.hasPrimaryClip() || manager.getPrimaryClip() == null
                    || manager.getPrimaryClip().getItemCount() == 0
                    || manager.getPrimaryClipDescription() == null
                    || !(manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                        || manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML))) {
                s.replyModel.setSource("");
            } else {
                CharSequence value = manager.getPrimaryClip().getItemAt(0).getText();
                s.replyModel.setSource(value == null ? "" : value.toString());
            }
        } catch (SecurityException | IllegalStateException error) {
            s.replyModel.invalidate("无法读取剪贴板，请重试");
        }
        s.clearReplyRequestReferences();
        renderReplyKeyboard();
    }

    void generateReply(String style) {
        if (s.aiPolishConfiguration == null) {
            s.replyModel.showStatus("请先在共享设置中启用并配置 AI 辅助");
            renderReplyKeyboard();
            return;
        }
        if (!s.replyReady()) {
            s.replyModel.showStatus("请先完成输入，再选择回复方式");
            renderReplyKeyboard();
            return;
        }
        java.util.List<CommunityReplyLibrary.Template> templates = java.util.List.of();
        if (style != null && style.startsWith("community:")) {
            try { templates = s.communityReplyLibrary == null ? java.util.List.of() : s.communityReplyLibrary.read(); }
            catch (java.io.IOException error) {
                s.replyModel.showStatus("回复模板无法读取，请重试");
                renderReplyKeyboard();
                return;
            }
        }
        ReplyKeyboardModel.Request request = s.replyModel.begin(style, templates);
        if (request == null) {
            renderReplyKeyboard();
            return;
        }
        s.replyRequestConfiguration = s.aiPolishConfiguration;
        s.replyTarget = new EditorContextSnapshot(s.connection, s.editorContextRevision, s.editorContext(true),
            s.selectedEditorText(), s.editorContext(false));
        renderReplyKeyboard();
        try {
            AiPolishConfiguration requestConfiguration = s.aiPolishConfiguration.withPrompt(request.prompt());
            s.replyOperation = s.aiPolishClient.request(requestConfiguration, request.source(),
                (generation, result, failure) -> s.main.post(
                    () -> s.finishReply(request.generation(), generation, result, failure)));
            s.replyModel.attachCancellation(request.generation(), s.replyOperation::cancel);
        } catch (AiPolishClient.Failure | IllegalArgumentException error) {
            s.replyModel.fail(request.generation(), "无法启动 AI 请求，请检查配置");
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        }
    }

    void useReply(String text) {
        boolean inserted = s.replyModel.use(text, value -> {
            if (!s.replyReady() || !s.replyTargetMatches() || s.replyRequestConfiguration == null
                    || !s.replyRequestConfiguration.equals(s.aiPolishConfiguration)
                    || s.connection == null) return false;
            try {
                if (!s.commitText(value, TypingSource.REPLY)) return false;
            } catch (RuntimeException error) { return false; }
            s.replyOpen = false;
            return true;
        });
        s.clearReplyRequestReferences();
        renderReplyKeyboard();
        if (inserted) {
            s.setReplyKeyboardVisible(false);
            s.render();
        }
    }

    void showReplyTemplates() {
        if (s.replyTemplateButton == null || s.replyModel.busy()) return;
        final java.util.List<CommunityReplyLibrary.Template> templates;
        try { templates = s.communityReplyLibrary == null ? java.util.List.of() : s.communityReplyLibrary.read(); }
        catch (java.io.IOException error) {
            s.replyModel.showStatus("回复模板无法读取，请重试");
            renderReplyKeyboard();
            return;
        }
        if (templates.isEmpty()) {
            Toast.makeText(s, "请先在 App 社区收藏并添加回复模板", Toast.LENGTH_SHORT).show();
            return;
        }
        PopupMenu popup = new PopupMenu(s, s.replyTemplateButton);
        for (CommunityReplyLibrary.Template template : templates) {
            popup.getMenu().add(template.name()).setOnMenuItemClickListener(ignored -> {
                generateReply("community:" + template.id());
                return true;
            });
        }
        popup.show();
    }

    void showSkinMenu(Button anchor) {
        if (anchor == null || !s.canSaveKeyboardSkin() || s.skinScroll == null) return;
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeMoreTools();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        renderSkinPicker();
        s.skinScroll.setVisibility(View.VISIBLE);
    }

    void renderSkinPicker() {
        if (s.skinPanel == null) return;
        s.skinPanel.removeAllViews();
        LinearLayout header = new LinearLayout(s);
        TextView title = new TextView(s);
        title.setText("选择皮肤");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button close = s.button(header, "返回键盘", s::closeSkinPicker);
        close.setContentDescription("返回键盘");
        s.skinPanel.addView(header);

        java.util.List<MSIMEInputService.SkinChoice> saved = new java.util.ArrayList<>();
        JSONObject preferences = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        boolean hostDark = KeyboardSkin.resolveDark(
            preferences == null ? "follow" : preferences.optString("screen_keyboard_theme", "follow"),
            preferences == null ? "system" : preferences.optString("theme", "system"), s.systemDark());
        try {
            for (CustomSkinLibrary.Item item : CustomSkinLibrary.read(java.nio.file.Paths.get(s.preferencesDirectory))) {
                JSONObject design = item.design();
                saved.add(new MSIMEInputService.SkinChoice("custom", item.name(), KeyboardSkin.custom(design, hostDark), design));
            }
        } catch (Exception ignored) {
            // A partially written library must not hide the themes.
        }
        if (!saved.isEmpty()) addSkinSection(s.skinPanel, "我的设计", saved);

        // The global themes in the shared catalog's order. A built-in card draws its catalog palette in the theme's own fixed mode; 跟随系统 draws the Material 3 tokens in this keyboard's mode; the custom card draws the custom theme as it stands, which is 我的皮肤 once a keyboard design exists.
        java.util.List<MSIMEInputService.SkinChoice> builtIns = new java.util.ArrayList<>();
        JSONObject customTheme = preferences == null ? null : preferences.optJSONObject("custom_theme");
        JSONArray themes = s.themeCatalog();
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = entry.optString("id", "");
            if (id.isEmpty()) continue;
            String themeName = entry.optString("title", id);
            KeyboardSkin choice = "custom".equals(id) ? s.themeSkin(id, customTheme, hostDark)
                : KeyboardSkin.resolved(entry, themeName, hostDark, null);
            builtIns.add(new MSIMEInputService.SkinChoice(id, choice.title(), choice, null));
        }
        if (builtIns.isEmpty()) {
            KeyboardSkin system = KeyboardSkin.system(hostDark);
            builtIns.add(new MSIMEInputService.SkinChoice(system.id(), system.title(), system, null));
        }
        addSkinSection(s.skinPanel, null, builtIns);
        s.imeStyler.applySkin();
    }

    void addSkinSection(LinearLayout parent, String heading, java.util.List<MSIMEInputService.SkinChoice> choices) {
        JSONObject stored = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        String globalTheme = stored == null ? "system" : stored.optString("global_theme", "system");
        if (heading != null) {
            TextView label = new TextView(s);
            label.setText(heading);
            label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
            parent.addView(label, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int start = 0; start < choices.size(); start += 2) {
            LinearLayout row = new LinearLayout(s);
            row.setOrientation(LinearLayout.HORIZONTAL);
            // 卡片是小布局，没有文字基线可对齐。
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 2; slot++) {
                int index = start + slot;
                if (index >= choices.size()) {
                    row.addView(new View(s), new LinearLayout.LayoutParams(0, s.pixels(124), 1));
                    continue;
                }
                MSIMEInputService.SkinChoice choice = choices.get(index);
                KeyboardSkinCard card = new KeyboardSkinCard(s, choice.skin(), choice.title());
                // A theme card is selected by the stored global theme; a saved design only while the custom theme draws exactly that design.
                card.setSelected(choice.design() == null ? choice.id().equals(globalTheme)
                    : "custom".equals(globalTheme) && s.skin.key().equals(choice.skin().key()));
                card.setContentDescription("屏幕键盘皮肤 " + choice.title());
                if (Build.VERSION.SDK_INT >= 30)
                    card.setStateDescription(card.isSelected() ? "已选中" : "未选中");
                card.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    s.closeSkinPicker();
                    s.saveKeyboardSkin(choice.id(), choice.design());
                });
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, s.pixels(124), 1);
                params.setMargins(s.pixels(4), s.pixels(4), s.pixels(4), s.pixels(4));
                row.addView(card, params);
            }
            parent.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(132)));
        }
    }

    /**
     * 高情商回复面板，布局照 iOS 的 `ReplyKeyboardView`：分段控件与模板按钮一行，源文字卡片一行，左边风格九宫格或回复列表、右边 60dp 操作列，最底下一行状态。
     *
     * <p>间距取键盘自己的键距和行距，圆角和底色取当前皮肤的键帽，所以浅色、深色和自定义皮肤下都与键区一致。九宫格和回复卡片用 `KEY` 角色交给皮肤遍历上色；分段控件、源文字卡片、行内「粘贴」和操作列由 {@link #styleReplyKeyboard()} 在皮肤遍历之后单独上色。
     */
    LinearLayout createReplyKeyboard() {
        LinearLayout root = new LinearLayout(s);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(s.pixels(6), s.pixels(5), s.pixels(6), s.pixels(5));
        root.setBackgroundColor(Color.parseColor(s.skin.background()));
        root.setContentDescription("高情商回复键盘");

        s.replyHeader = new LinearLayout(s);
        s.replyHeader.setGravity(Gravity.CENTER_VERTICAL);
        s.replyModeControl = new LinearLayout(s);
        s.replyModeControl.setPadding(s.pixels(2), s.pixels(2), s.pixels(2), s.pixels(2));
        s.replyReplyModeButton = replySegment("帮你回", "帮你回模式", ReplyKeyboardModel.Mode.REPLY);
        s.replyPolishModeButton = replySegment("帮润色", "帮润色模式", ReplyKeyboardModel.Mode.POLISH);
        s.replyHeader.addView(s.replyModeControl, new LinearLayout.LayoutParams(
            s.pixels(200), LinearLayout.LayoutParams.MATCH_PARENT));
        s.replyHeader.addView(new View(s), new LinearLayout.LayoutParams(0, 1, 1));
        s.replyTemplateButton = s.shortcutButton(s.replyHeader, "模板",
            KeyboardShortcutIconPolicy.Icon.BOOKMARK, this::showReplyTemplates);
        s.replyTemplateButton.setContentDescription("回复模板");
        s.replyTemplateButton.setLayoutParams(new LinearLayout.LayoutParams(
            s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyHeader, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(36)));

        // 源文字和「粘贴」在同一张卡片里：点文字和点「粘贴」都是粘贴，与 iOS 相同。
        s.replySourceCard = new LinearLayout(s);
        s.replySourceCard.setGravity(Gravity.CENTER_VERTICAL);
        s.replySourceCard.setPadding(s.pixels(10), 0, s.pixels(6), 0);
        s.replySourceButton = MSIMEInputService.role(s.button(s.replySourceCard, MSIMEInputService.REPLY_SOURCE_PLACEHOLDER,
            this::pasteReplySource), KeyboardKeyRole.PLAIN);
        s.replySourceButton.setSingleLine(true);
        s.replySourceButton.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.replySourceButton.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
        s.replySourceButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        s.replySourceButton.setContentDescription("回复源文字");
        compactReplyControl(s.replySourceButton, 0);
        s.replySourceButton.setLayoutParams(new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        s.replyPasteButton = MSIMEInputService.role(s.button(s.replySourceCard, "粘贴", this::pasteReplySource),
            KeyboardKeyRole.PLAIN);
        s.replyPasteButton.setContentDescription("粘贴回复源文字");
        s.replyPasteButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        compactReplyControl(s.replyPasteButton, s.pixels(10));
        LinearLayout.LayoutParams pasteParams = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT);
        pasteParams.setMarginStart(s.pixels(6));
        s.replyPasteButton.setLayoutParams(pasteParams);
        root.addView(s.replySourceCard, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(38)));

        s.replyBody = new LinearLayout(s);
        s.replyScroll = new ScrollView(s);
        s.replyScroll.setFillViewport(true);
        s.replyScroll.setVerticalScrollBarEnabled(false);
        s.replyMain = new LinearLayout(s);
        s.replyMain.setOrientation(LinearLayout.VERTICAL);
        s.replyMain.setContentDescription("回复风格与候选");
        s.replyScroll.addView(s.replyMain);
        s.replyBody.addView(s.replyScroll, new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        s.replyActions = new LinearLayout(s);
        s.replyActions.setOrientation(LinearLayout.VERTICAL);
        s.replyBody.addView(s.replyActions, new LinearLayout.LayoutParams(
            s.pixels(60), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyBody, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));

        LinearLayout footer = new LinearLayout(s);
        footer.setGravity(Gravity.CENTER_VERTICAL);
        s.replyProgress = new android.widget.ProgressBar(s, null,
            android.R.attr.progressBarStyleSmall);
        s.replyProgress.setIndeterminate(true);
        s.replyProgress.setVisibility(View.GONE);
        LinearLayout.LayoutParams progressParams = new LinearLayout.LayoutParams(
            s.pixels(12), s.pixels(12));
        progressParams.setMarginEnd(s.pixels(4));
        footer.addView(s.replyProgress, progressParams);
        s.replyStatus = new TextView(s);
        s.replyStatus.setSingleLine(true);
        s.replyStatus.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.replyStatus.setIncludeFontPadding(false);
        s.replyStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11);
        s.replyStatus.setContentDescription("高情商回复键盘状态");
        footer.addView(s.replyStatus, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        // 「选风格」只在已有回复时出现，点它回到风格九宫格。
        s.replyStyleResetButton = MSIMEInputService.role(s.button(footer, "选风格", () -> {
            s.replyModel.chooseStyle();
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.GLYPH);
        s.replyStyleResetButton.setContentDescription("重新选择回复风格");
        s.replyStyleResetButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        compactReplyControl(s.replyStyleResetButton, s.pixels(6));
        s.replyStyleResetButton.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.replyStyleResetButton.setVisibility(View.GONE);
        root.addView(footer, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(18)));
        return root;
    }

    Button replySegment(String label, String description, ReplyKeyboardModel.Mode mode) {
        Button segment = MSIMEInputService.role(s.button(s.replyModeControl, label, () -> {
            s.replyModel.setMode(mode);
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.PLAIN);
        segment.setContentDescription(description);
        segment.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        compactReplyControl(segment, 0);
        segment.setLayoutParams(new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        return segment;
    }

    /** 去掉 Button 自带的最小尺寸、内边距和按下抬升，让回复面板里的控件按自己给定的尺寸排布。 */
    static void compactReplyControl(Button button, int horizontalPadding) {
        button.setMinWidth(0);
        button.setMinimumWidth(0);
        button.setMinHeight(0);
        button.setMinimumHeight(0);
        button.setPadding(horizontalPadding, 0, horizontalPadding, 0);
        button.setIncludeFontPadding(false);
        button.setStateListAnimator(null);
    }

    Button replyAction(String label, String description, Runnable action) {
        Button button = MSIMEInputService.role(s.button(s.replyActions, label, action), KeyboardKeyRole.PLAIN);
        button.setContentDescription(description);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        compactReplyControl(button, 0);
        button.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        return button;
    }

    void renderReplyKeyboard() {
        if (s.replyKeyboard == null || s.replyMain == null || s.replyActions == null) return;
        s.replyMain.removeAllViews();
        s.replyActions.removeAllViews();
        boolean busy = s.replyModel.busy();
        boolean hasReplies = !s.replyModel.replies().isEmpty();
        selectReplySegment(s.replyReplyModeButton, s.replyModel.mode() == ReplyKeyboardModel.Mode.REPLY);
        selectReplySegment(s.replyPolishModeButton, s.replyModel.mode() == ReplyKeyboardModel.Mode.POLISH);
        s.replyTemplateButton.setEnabled(!busy);
        s.replySourceButton.setText(s.replyModel.source().isEmpty()
            ? MSIMEInputService.REPLY_SOURCE_PLACEHOLDER : s.replyModel.source());
        if (!hasReplies) {
            for (int start = 0; start < ReplyKeyboardModel.STYLES.size(); start += 3) {
                LinearLayout row = new LinearLayout(s);
                for (int column = 0; column < 3; column++) {
                    ReplyKeyboardModel.Style style = ReplyKeyboardModel.STYLES.get(start + column);
                    Button choice = MSIMEInputService.role(s.button(row, style.emoji() + " " + style.label(),
                        () -> generateReply(style.label())), KeyboardKeyRole.KEY);
                    choice.setContentDescription("回复风格 " + style.label());
                    choice.setEnabled(!busy);
                    choice.setAlpha(busy ? .45f : 1f);
                    choice.setMinWidth(0);
                    choice.setMinimumWidth(0);
                    choice.setMinHeight(0);
                    choice.setMinimumHeight(0);
                    choice.setPadding(s.pixels(4), 0, s.pixels(4), 0);
                    choice.setMaxLines(1);
                    choice.setAutoSizeTextTypeUniformWithConfiguration(
                        10, 14, 1, TypedValue.COMPLEX_UNIT_SP);
                    choice.setLayoutParams(new LinearLayout.LayoutParams(
                        0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
                }
                s.replyMain.addView(row, new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
            }
        } else {
            for (String reply : s.replyModel.replies()) {
                Button candidate = MSIMEInputService.role(s.button(s.replyMain, reply, () -> useReply(reply)),
                    KeyboardKeyRole.KEY);
                candidate.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
                candidate.setContentDescription("回复候选，点按插入");
                candidate.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
                candidate.setMinWidth(0);
                candidate.setMinimumWidth(0);
                candidate.setMinHeight(0);
                candidate.setMinimumHeight(0);
                candidate.setPadding(s.pixels(10), s.pixels(10), s.pixels(10), s.pixels(10));
                candidate.setLayoutParams(new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            }
        }
        replyAction("⌫", "删除源文字", () -> {
            s.replyModel.deleteLastCodePoint();
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        });
        replyAction("清空", "清空源文字", () -> {
            s.replyModel.setSource("");
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        });
        if (busy) {
            s.replyPrimaryAction = null;
            replyAction("取消", "取消回复生成", () -> {
                s.replyModel.cancel();
                s.clearReplyRequestReferences();
                renderReplyKeyboard();
            });
        } else if (!hasReplies) {
            s.replyPrimaryAction = replyAction("生成", "生成回复",
                () -> generateReply(s.replyModel.style()));
        } else {
            s.replyPrimaryAction = replyAction("换一句", "换一句回复",
                () -> generateReply(s.replyModel.style()));
        }
        s.replyStatus.setText(s.replyModel.status());
        s.replyProgress.setVisibility(busy ? View.VISIBLE : View.GONE);
        s.replyStyleResetButton.setVisibility(hasReplies ? View.VISIBLE : View.GONE);
        applyReplyGeometry();
        s.imeStyler.applySkinToView(s.replyKeyboard);
        styleReplyKeyboard();
    }

    static void selectReplySegment(Button segment, boolean selected) {
        segment.setSelected(selected);
        if (Build.VERSION.SDK_INT >= 30)
            segment.setStateDescription(selected ? "已选中" : "未选中");
    }

    /** 回复面板的格间距：列间用键距、行间用行距，与键区同一组设置，改设置后随键区一起更新。 */
    void applyReplyGeometry() {
        if (s.replyKeyboard == null || s.replyMain == null) return;
        int keyGap = s.halfSpacingPixels(s.touchKeySpacingTenths) * 2;
        int rowGap = s.halfSpacingPixels(s.touchRowSpacingTenths) * 2;
        spaceReplyChildren(s.replyKeyboard, rowGap);
        spaceReplyChildren(s.replyHeader, keyGap);
        spaceReplyChildren(s.replyBody, keyGap);
        spaceReplyChildren(s.replyActions, rowGap);
        boolean grid = s.replyModel.replies().isEmpty();
        // 九宫格行间用行距；回复卡片纵向排列，iOS 在这里用的是键距。
        spaceReplyChildren(s.replyMain, grid ? rowGap : keyGap);
        for (int index = 0; index < s.replyMain.getChildCount(); index++) {
            if (s.replyMain.getChildAt(index) instanceof LinearLayout row)
                spaceReplyChildren(row, keyGap);
        }
    }

    static void spaceReplyChildren(LinearLayout layout, int gap) {
        GradientDrawable divider = new GradientDrawable();
        divider.setColor(Color.TRANSPARENT);
        divider.setSize(gap, gap);
        layout.setDividerDrawable(divider);
        layout.setShowDividers(LinearLayout.SHOW_DIVIDER_MIDDLE);
    }

    static GradientDrawable replySurface(int color, float radius) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(color);
        drawable.setCornerRadius(radius);
        return drawable;
    }

    /**
     * 给皮肤遍历不负责的回复控件上色，必须在 `applySkinToView` 之后调用：那一遍会把 `PLAIN` 按钮的底色清空，把选中的按钮画成实心强调色。
     *
     * <p>分段控件照 iOS 的分段样式：整体一条半透明底，选中的一段铺键帽底色、字加粗。操作列的底色是键帽的 70%，主操作（生成、换一句）用强调色，好和删除、清空区分开。
     */
    void styleReplyKeyboard() {
        if (s.replyKeyboard == null || s.replyModeControl == null) return;
        float radius = s.pixels(s.skin.cornerRadius());
        int foreground = Color.parseColor(s.skin.keyForeground());
        int accent = Color.parseColor(s.skin.accent());
        int onAccent = Color.parseColor(s.skin.onAccent());
        Typeface base = s.skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT;
        s.replyModeControl.setBackground(replySurface(ImeStyler.fade(s.skin.keyForeground(), .08), radius));
        for (Button segment : new Button[] {s.replyReplyModeButton, s.replyPolishModeButton}) {
            boolean selected = segment.isSelected();
            segment.setBackground(selected ? replySurface(Color.parseColor(s.skin.keyBackground()),
                Math.max(0, radius - s.pixels(2))) : null);
            segment.setTextColor(foreground);
            segment.setTypeface(Typeface.create(base, selected ? Typeface.BOLD : Typeface.NORMAL));
            segment.setElevation(0);
        }
        s.replySourceCard.setBackground(replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        s.replySourceButton.setTextColor(s.replyModel.source().isEmpty()
            ? ImeStyler.fade(s.skin.keyForeground(), .55) : foreground);
        s.replyPasteButton.setBackground(new InsetDrawable(replySurface(accent, radius),
            0, s.pixels(6), 0, s.pixels(6)));
        // setBackground 会把 InsetDrawable 的内边距（左右为 0）套到按钮上，冲掉前面设的左右留白，文字就贴着色块边缘；换完背景再设回来。
        s.replyPasteButton.setPadding(s.pixels(12), 0, s.pixels(12), 0);
        s.replyPasteButton.setTextColor(onAccent);
        s.replyPasteButton.setElevation(0);
        for (int index = 0; index < s.replyActions.getChildCount(); index++) {
            if (!(s.replyActions.getChildAt(index) instanceof Button action)) continue;
            boolean primary = action == s.replyPrimaryAction;
            action.setBackground(replySurface(primary ? accent : ImeStyler.fade(s.skin.keyBackground(), .7), radius));
            action.setTextColor(primary ? onAccent : foreground);
            action.setElevation(0);
        }
        s.replyStatus.setTextColor(ImeStyler.fade(s.skin.keyForeground(), .7));
        s.replyProgress.setIndeterminateTintList(ColorStateList.valueOf(accent));
    }

    void showAiPolish() {
        if (s.aiPolishConfiguration == null) {
            Toast.makeText(s, "请先在共享设置中启用并配置 AI 辅助", Toast.LENGTH_SHORT).show();
            return;
        }
        String selected = s.selectedEditorText();
        if (!s.aiPolishReady() || !AiPolishConfiguration.acceptableText(selected)) {
            Toast.makeText(s, "请先完成当前输入，再选择一万字以内的文字", Toast.LENGTH_SHORT).show();
            return;
        }
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        s.aiRequestConfiguration = s.aiPolishConfiguration;
        s.aiSourceText = selected;
        s.aiTarget = new EditorContextSnapshot(s.connection, s.editorContextRevision, s.editorContext(true),
            selected, s.editorContext(false));
        renderAiPolish();
        s.aiPolishContainer.setVisibility(View.VISIBLE);
    }

    void sendAiPolish() {
        if (s.aiBusy || s.aiRequestConfiguration == null || !s.aiTargetMatches()
                || !s.aiRequestConfiguration.equals(s.aiPolishConfiguration)) {
            s.aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
            renderAiPolish();
            return;
        }
        s.aiBusy = true;
        s.aiError = "";
        renderAiPolish();
        try {
            s.aiOperation = s.aiPolishClient.request(s.aiRequestConfiguration, s.aiSourceText,
                (generation, result, failure) -> s.main.post(
                    () -> finishAiPolish(generation, result, failure)));
        } catch (AiPolishClient.Failure error) {
            s.aiBusy = false;
            s.aiError = error.reason() == AiPolishClient.Reason.BUSY
                ? "已有 AI 请求正在处理，请稍后重试。" : "无法启动 AI 请求，请检查配置。";
            renderAiPolish();
        }
    }

    void finishAiPolish(long generation, String result, AiPolishClient.Failure failure) {
        if (s.aiOperation == null || s.aiOperation.generation() != generation
                || s.aiPolishContainer == null
                || s.aiPolishContainer.getVisibility() != View.VISIBLE) return;
        s.aiOperation = null;
        s.aiBusy = false;
        if (!s.aiTargetMatches() || s.aiRequestConfiguration == null
                || !s.aiRequestConfiguration.equals(s.aiPolishConfiguration)) {
            s.aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
        } else if (failure != null) {
            s.aiError = failure.reason() == AiPolishClient.Reason.INVALID
                ? "服务返回的文字为空或超过一万字。"
                : "AI 请求失败，请检查网络、地址、模型和密钥。";
        } else {
            s.aiOutputText = result;
            s.aiError = "";
        }
        renderAiPolish();
    }

    void replaceAiSelection() {
        if (s.aiOutputText.isEmpty() || !s.aiPolishReady() || !s.aiTargetMatches()
                || s.aiRequestConfiguration == null
                || !s.aiRequestConfiguration.equals(s.aiPolishConfiguration)) {
            s.aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
            renderAiPolish();
            return;
        }
        boolean committed;
        committed = s.commitText(s.aiOutputText, TypingSource.AI);
        if (committed) s.closeAiPolish();
        else {
            s.aiError = "编辑器拒绝替换，请返回键盘后重试。";
            renderAiPolish();
        }
    }

    void renderAiPolish() {
        if (s.aiPolishPanel == null || s.aiPolishActions == null) return;
        s.aiPolishPanel.removeAllViews();
        s.aiPolishActions.removeAllViews();
        LinearLayout header = new LinearLayout(s);
        TextView title = new TextView(s);
        title.setText("AI 润色");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        s.button(header, "返回键盘", s::closeAiPolish);
        s.aiPolishPanel.addView(header);
        if (!s.aiError.isEmpty()) {
            TextView error = new TextView(s);
            error.setText(s.aiError);
            error.setTextColor(Color.RED);
            error.setContentDescription("AI 润色状态");
            s.aiPolishPanel.addView(error);
        }
        if (s.aiRequestConfiguration != null) {
            TextView destination = new TextView(s);
            destination.setText("发送到 " + s.aiRequestConfiguration.destination() + " · "
                + s.aiRequestConfiguration.model());
            destination.setContentDescription("AI 请求目标和模型");
            s.aiPolishPanel.addView(destination);
        }
        TextView label = new TextView(s);
        label.setText(s.aiOutputText.isEmpty() ? "待发送的选中文字" : "润色结果");
        s.aiPolishPanel.addView(label);
        TextView content = new TextView(s);
        content.setText(s.aiOutputText.isEmpty() ? s.aiSourceText : s.aiOutputText);
        content.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        content.setContentDescription(s.aiOutputText.isEmpty() ? "待润色文字" : "AI 润色结果");
        s.aiPolishPanel.addView(content);
        if (s.aiBusy) {
            TextView progress = new TextView(s);
            progress.setText("正在请求…");
            s.aiPolishPanel.addView(progress);
            Button cancel = s.button(s.aiPolishActions, "取消请求", () -> {
                s.cancelAiRequest();
                renderAiPolish();
            });
            cancel.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        } else if (s.aiOutputText.isEmpty()) {
            Button send = s.button(s.aiPolishActions, "发送选中文字", this::sendAiPolish);
            send.setEnabled(s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
            send.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        } else {
            Button replace = s.button(s.aiPolishActions, "替换选中文字", this::replaceAiSelection);
            replace.setEnabled(s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
            replace.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        s.imeStyler.applySkin();
    }

    void showSchemePicker() {
        if (s.touchGeometrySaving || s.traditionalOutputSaving
                || s.session == 0 || s.preferencesSnapshot == null
                || s.preferencesDirectory.isEmpty()) {
            Toast.makeText(s, "输入方案尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeLayoutSettings();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        renderSchemePicker();
        s.schemeScroll.setVisibility(View.VISIBLE);
    }

    void renderSchemePicker() {
        if (s.schemePanel == null) return;
        s.schemePanel.removeAllViews();
        LinearLayout header = new LinearLayout(s);
        // 标题去掉了：这一屏只有方案卡片，左上返回、右上设置，和母版一致。写着「输入方案」的那行
        // 字和那颗「返回键盘」按钮，占的是卡片的位置，说的却是用户已经看见的事。
        Button close = s.borderlessButton(header, "‹", s::closeSchemePicker);
        close.setContentDescription("返回键盘");
        // 高度写 0，不写 WRAP_CONTENT：裸 View 的默认测量在 AT_MOST 下取满可用空间，这一条
        // 占位会把标题栏撑到整屏高，卡片区就一点高度都分不到了。
        header.addView(new View(s), new LinearLayout.LayoutParams(0, 0, 1));
        Button settings = s.borderlessButton(header, "⚙", this::showFeedbackMenu);
        settings.setContentDescription("键盘设置");
        s.schemePanel.addView(header);
        LinearLayout schemeSurface = new LinearLayout(s);
        schemeSurface.setOrientation(LinearLayout.VERTICAL);
        schemeSurface.setPadding(s.pixels(8), s.pixels(6), s.pixels(8), s.pixels(6));
        schemeSurface.setContentDescription("输入方案卡片区域");
        // 卡面按内容高度收，不再撑满标题以下的全部空间。撑满原本是为了「短列表下面不要露出
        // 键盘底纹」，但十三张卡片也填不满一屏，结果是一大块什么都没有的白。露出的是选择器
        // 自己的底色，与卡片同一套配色，比那块空白好看。
        s.schemePanel.addView(schemeSurface, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        java.util.List<KeyboardScheme> schemes = s.visibleSchemes;
        int cardCount = schemes.size() + 1;
        // The English card sits third when there are enough schemes to put it there, and last
        // otherwise. Pinning it to index 2 made a single enabled scheme index past the end of
        // the list, which threw on the main thread and took the IME down with it.
        final int englishIndex = Math.min(2, schemes.size());
        java.util.List<KeyboardSchemeCard> schemeCards = new java.util.ArrayList<>();
        java.util.List<Boolean> cardSelection = new java.util.ArrayList<>();
        for (int start = 0; start < cardCount; start += 4) {
            LinearLayout row = new LinearLayout(s);
            row.setOrientation(LinearLayout.HORIZONTAL);
            // 卡片是小布局，没有文字基线可对齐。
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 4; slot++) {
                int index = start + slot;
                if (index >= cardCount) {
                    View spacer = new View(s);
                    row.addView(spacer, new LinearLayout.LayoutParams(0, s.pixels(72), 1));
                    continue;
                }
                if (index == englishIndex) {
                    // English is a platform text mode, not a second persisted Engine scheme.
                    KeyboardSchemeCard card = new KeyboardSchemeCard(
                        s, "EN", "26", "英文 26 键");
                    card.setOnClickListener(ignored -> {
                        s.imeKeyFeedback.playFeedback(card);
                        s.selectEnglishScheme();
                    });
                    row.addView(card, new LinearLayout.LayoutParams(0, s.pixels(72), 1));
                    card.setEnabled(!s.schemeSaving);
                    card.setContentDescription("输入方案卡片 英文 26 键");
                    if (Build.VERSION.SDK_INT >= 30)
                        card.setStateDescription(s.dedicatedEnglish ? "已选中" : "未选中");
                    schemeCards.add(card);
                    cardSelection.add(s.dedicatedEnglish);
                    continue;
                }
                int schemeIndex = index > englishIndex ? index - 1 : index;
                KeyboardScheme scheme = schemes.get(schemeIndex);
                // Apple renders scheme cards with the same press-feedback surface as keys. Keep
                // the Android-specific scheme persistence and selection guards in the callback.
                KeyboardSchemeCard card = new KeyboardSchemeCard(
                    s, scheme.glyph(), scheme.badge(s.wubiProfile), scheme.title(s.wubiProfile));
                card.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    s.selectKeyboardScheme(scheme);
                });
                row.addView(card, new LinearLayout.LayoutParams(0, s.pixels(72), 1));
                card.setEnabled(!s.schemeSaving);
                card.setContentDescription("输入方案卡片 " + scheme.title(s.wubiProfile));
                if (Build.VERSION.SDK_INT >= 30)
                    card.setStateDescription(scheme == s.selectedScheme ? "已选中" : "未选中");
                schemeCards.add(card);
                cardSelection.add(scheme == s.selectedScheme);
            }
            schemeSurface.addView(row);
        }
        s.imeStyler.applySkin();
        // Apple keeps the selectable scheme area on a filled key surface, so a short list does not
        // leave a bare keyboard backdrop below the cards. Apply this after the recursive skin pass:
        // the picker itself remains the patterned backdrop while this inner surface follows the
        // selected skin's key material, including custom Android skins.
        int cardSurface = Color.parseColor(s.skin.keyBackground());
        schemeSurface.setBackground(new KeyboardSkinKeyDrawable(s.skin, cardSurface, false,
            s.getResources().getDisplayMetrics().density));
        // 也必须在那一趟之后：它会把每个 TextView 重新刷成 keyForeground，卡片的强调色先上就没了。
        int cardAccent = Color.parseColor(s.skin.accent());
        for (int index = 0; index < schemeCards.size(); index++) {
            schemeCards.get(index).paint(cardAccent, cardSurface, cardSelection.get(index));
        }
    }

    void showClipboardHistory() {
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!CloudClipboardPanelPolicy.panelAvailable(s.clipboardHistoryEnabled, cloudAllowed)
                || s.clipboardScroll == null) return;
        // Clipboard entries are independent editor text. Finish the active composition when the
        // panel opens, matching the symbol and emoji panels instead of leaving stale preedit behind
        // while the user browses history.
        s.command(2);
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.clipboardTab = CloudClipboardPanelPolicy.initialTab(
            s.clipboardTab, s.clipboardHistoryEnabled, cloudAllowed);
        renderClipboardHistory();
        s.clipboardScroll.setVisibility(View.VISIBLE);
        // Fetched on every opening, whichever half is showing: the local half's 发到云剪贴板 needs to know the account is signed in with the cloud clipboard on.
        if (cloudAllowed) refreshCloudClipboard();
    }

    boolean clipboardPanelOpen() {
        return s.clipboardScroll != null && s.clipboardScroll.getVisibility() == View.VISIBLE;
    }

    boolean cloudClipboardAllowed() {
        return CloudClipboardPanelPolicy.cloudAllowed(s.editorInputType, s.allowLearning);
    }

    void selectClipboardTab(CloudClipboardPanelPolicy.Tab tab) {
        if (tab == CloudClipboardPanelPolicy.Tab.CLOUD && !cloudClipboardAllowed()) return;
        s.clipboardTab = tab;
        renderClipboardHistory();
    }

    /**
     * Ask the service for this account's cloud list, off the main thread.
     *
     * <p>The answer is drawn only if the field and the panel are still the ones it was asked for; otherwise it is dropped. Errors carry no response text, and nothing about the request is logged.
     */
    void refreshCloudClipboard() {
        if (!cloudClipboardAllowed()) return;
        long generation = ++s.cloudClipboardGeneration;
        s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
        s.cloudClipboardItems = java.util.List.of();
        if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
        try {
            s.cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status status;
                java.util.List<BackendAccount.ClipboardItem> items = java.util.List.of();
                try {
                    BackendAccount account = new BackendAccount(s);
                    // Throws when the session owner cannot tell right now, which is a retry, not a sign-in.
                    if (account.currentAccessToken().isEmpty()) {
                        status = CloudClipboardPanelPolicy.Status.SIGNED_OUT;
                    } else {
                        BackendAccount.ClipboardPage page = account.clipboard("");
                        status = CloudClipboardPanelPolicy.loaded(page.enabled(), page.items().size());
                        if (CloudClipboardPanelPolicy.showsItems(status)) items = page.items();
                    }
                } catch (BackendAccount.RequestException error) {
                    status = CloudClipboardPanelPolicy.failed(error.status);
                } catch (Exception | LinkageError error) {
                    status = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status answer = status;
                java.util.List<BackendAccount.ClipboardItem> answered = items;
                s.main.post(() -> {
                    if (!CloudClipboardPanelPolicy.accepts(generation, s.cloudClipboardGeneration)
                            || !clipboardPanelOpen() || !cloudClipboardAllowed()) return;
                    s.cloudClipboardStatus = answer;
                    s.cloudClipboardItems = answered;
                    if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.FAILED;
        }
    }

    /** Send one local entry to the account's cloud clipboard, because the user asked for exactly this one. */
    void uploadClipboardText(String text) {
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!CloudClipboardPanelPolicy.canUpload(cloudAllowed, s.cloudClipboardStatus, text)) {
            Toast.makeText(s, s.cloudClipboardStatus == CloudClipboardPanelPolicy.Status.SIGNED_OUT
                    || s.cloudClipboardStatus == CloudClipboardPanelPolicy.Status.DISABLED
                    ? CloudClipboardPanelPolicy.message(s.cloudClipboardStatus, 0)
                    : "这条记录无法发到云剪贴板", Toast.LENGTH_SHORT).show();
            return;
        }
        long generation = s.cloudClipboardGeneration;
        try {
            s.cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status failure = null;
                try {
                    new BackendAccount(s).addClipboard(text);
                } catch (BackendAccount.RequestException error) {
                    failure = CloudClipboardPanelPolicy.failed(error.status);
                } catch (Exception | LinkageError error) {
                    failure = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status result = failure;
                s.main.post(() -> {
                    if (!CloudClipboardPanelPolicy.acceptsUploadResult(
                            generation, s.cloudClipboardGeneration) || !clipboardPanelOpen()) return;
                    Toast.makeText(s, result == null ? "已发到云剪贴板"
                        : result == CloudClipboardPanelPolicy.Status.SIGNED_OUT
                            ? CloudClipboardPanelPolicy.SIGNED_OUT_MESSAGE
                            : "未能发到云剪贴板，请稍后重试", Toast.LENGTH_SHORT).show();
                    // Re-read rather than splice the entry in: the service deduplicates and orders the list.
                    refreshCloudClipboard();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            Toast.makeText(s, "未能发到云剪贴板，请稍后重试", Toast.LENGTH_SHORT).show();
        }
    }

    void insertCloudClipboardText(String text) {
        // Re-checked at the tap: the list was drawn for this field, but a field never gets cloud text once it has turned sensitive.
        if (!cloudClipboardAllowed()) return;
        s.insertClipboardText(text);
    }

    void renderClipboardHistory() {
        if (s.clipboardPanel == null || s.clipboardHistory == null) return;
        s.clipboardPanel.removeAllViews();
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!cloudAllowed) s.clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        boolean cloud = s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD;
        LinearLayout header = new LinearLayout(s);
        TextView title = new TextView(s);
        title.setText("剪贴板历史");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        if (cloud) {
            Button refresh = s.button(header, "刷新", this::refreshCloudClipboard);
            refresh.setEnabled(s.cloudClipboardStatus != CloudClipboardPanelPolicy.Status.LOADING);
            refresh.setContentDescription("刷新云剪贴板");
        } else if (s.clipboardHistoryEnabled) {
            s.button(header, "清空", s::confirmClearClipboardHistory);
        }
        s.button(header, "返回", s::closeClipboardHistory);
        s.clipboardPanel.addView(header);
        if (cloudAllowed) {
            LinearLayout tabs = new LinearLayout(s);
            addClipboardTab(tabs, CloudClipboardPanelPolicy.TAB_LOCAL, CloudClipboardPanelPolicy.Tab.LOCAL);
            addClipboardTab(tabs, CloudClipboardPanelPolicy.TAB_CLOUD, CloudClipboardPanelPolicy.Tab.CLOUD);
            s.clipboardPanel.addView(tabs);
        }
        if (cloud) {
            renderCloudClipboard();
            s.imeStyler.applySkin();
            return;
        }
        if (!s.clipboardHistoryEnabled) {
            TextView status = new TextView(s);
            status.setText("剪贴板历史未开启，可在设置中开启");
            s.clipboardPanel.addView(status);
            s.imeStyler.applySkin();
            return;
        }
        Button capture = s.button(s.clipboardPanel, "保存当前剪贴板", s::captureClipboardText);
        capture.setContentDescription("保存当前剪贴板文本");
        try {
            java.util.List<ClipboardHistory.Item> items = s.clipboardHistory.load();
            TextView status = new TextView(s);
            // Apple names the affordance next to the count; on Android the pin and delete actions
            // are behind the row's 管理 button, so that is what the hint points at.
            status.setText(items.isEmpty() ? "暂无历史 · 保存后点按插入 · 记录仅保存在本机"
                : items.size() + "/" + ClipboardHistoryPolicy.LIMIT
                    + " 条 · 点按插入 · 管理可固定或删除");
            s.clipboardPanel.addView(status);
            for (ClipboardHistory.Item item : items) {
                LinearLayout row = new LinearLayout(s);
                Button insert = s.button(row, item.text(), () -> s.insertClipboardText(item.text()));
                insert.setContentDescription((item.pinned() ? "已固定；" : "") + "点按插入剪贴板记录");
                Button manage = s.button(row, item.pinned() ? "已固定" : "管理", () -> {});
                manage.setOnClickListener(ignored -> s.manageClipboardItem(manage, item));
                row.getChildAt(0).setLayoutParams(new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1));
                row.getChildAt(1).setLayoutParams(new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
                s.clipboardPanel.addView(row);
            }
        } catch (IllegalStateException error) {
            TextView status = new TextView(s);
            status.setText("历史记录无法读取，请清空后重试");
            s.clipboardPanel.addView(status);
        }
        s.imeStyler.applySkin();
    }

    void addClipboardTab(LinearLayout tabs, String title, CloudClipboardPanelPolicy.Tab tab) {
        Button button = s.button(tabs, title, () -> selectClipboardTab(tab));
        button.setSelected(s.clipboardTab == tab);
        button.setContentDescription("剪贴板分类 " + title);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(button.isSelected() ? "已选中" : "未选中");
        s.imeStyler.styleButton(button, true);
    }

    void renderCloudClipboard() {
        TextView status = new TextView(s);
        status.setText(CloudClipboardPanelPolicy.message(s.cloudClipboardStatus, s.cloudClipboardItems.size()));
        s.clipboardPanel.addView(status);
        if (!CloudClipboardPanelPolicy.showsItems(s.cloudClipboardStatus)) return;
        for (BackendAccount.ClipboardItem item : s.cloudClipboardItems) {
            LinearLayout row = new LinearLayout(s);
            Button insert = s.button(row, item.text(), () -> insertCloudClipboardText(item.text()));
            insert.setContentDescription("点按插入云剪贴板记录");
            s.clipboardPanel.addView(row);
        }
    }

    void showFeedbackMenu() {
        if (s.moreButton == null || s.moreToolsPanel == null || s.moreToolsScroll == null) return;
        s.closeEmojiPicker();
        s.closeSymbolPanel();
        s.closeCandidatePanel();
        s.closeClipboardHistory();
        s.closeSchemePicker();
        s.closeLayoutSettings();
        s.closeVoiceResult();
        s.closeAiPolish();
        s.closeReplyKeyboard();
        s.localInputToolsOpen = false;
        s.imeFunctionPanel.renderMoreTools();
        s.moreToolsScroll.setVisibility(View.VISIBLE);
        s.moreToolsScroll.requestFocus();
    }

    void buildSymbolPanel() {
        s.symbolPanel = new SymbolPanelView(s,
            (title, description, action, actionStyle) -> {
                Button button = new KeyboardPressButton(s);
                button.setAllCaps(false);
                button.setText(title);
                button.setContentDescription(actionStyle ? description : "按键 " + description);
                s.imeStyler.styleButton(button, actionStyle);
                button.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(button);
                    action.run();
                });
                return button;
            },
            new SymbolPanelView.Listener() {
                @Override public void insert(String text) {
                    if (s.connection != null) s.commitText(text, TypingSource.LOCAL);
                }

                @Override public void delete() {
                    if (s.connection != null && !s.command(0))
                        s.deleteCodePointBeforeCursor();
                }

                @Override public void close() { s.closeSymbolPanel(); }
            });
        s.symbolPanel.setVisibility(View.GONE);
        s.keyboardSurface.addView(s.symbolPanel, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
    }

    void buildEmojiPanel() {
        s.emojiPreferences = s.getSharedPreferences(MSIMEInputService.EMOJI_RECENTS_PREFERENCES, Context.MODE_PRIVATE);
        s.emojiRecents = s.loadEmojiRecents();
        s.emojiPanel = new LinearLayout(s);
        s.emojiPanel.setOrientation(LinearLayout.VERTICAL);
        s.emojiPanel.setPadding(s.pixels(8), 0, s.pixels(8), s.pixels(6));
        s.emojiPanel.setBackgroundColor(Color.parseColor(s.skin.background()));
        s.emojiPanel.setContentDescription("表情面板");
        s.emojiPanel.setFocusable(true);
        LinearLayout emojiHeader = new LinearLayout(s);
        emojiHeader.setGravity(Gravity.CENTER_VERTICAL);
        Button closeEmoji = s.button(emojiHeader, "‹", s::closeEmojiPicker);
        ((KeyboardPressButton) closeEmoji).setKeyboardRole(KeyboardKeyRole.GLYPH);
        closeEmoji.setTextSize(TypedValue.COMPLEX_UNIT_SP, 26);
        closeEmoji.setPadding(0, 0, 0, s.pixels(3));
        closeEmoji.setMinHeight(0);
        closeEmoji.setMinimumHeight(0);
        closeEmoji.setContentDescription("返回键盘");
        closeEmoji.setLayoutParams(new LinearLayout.LayoutParams(s.pixels(48), s.pixels(40)));
        TextView emojiTitle = new TextView(s);
        emojiTitle.setText("表情");
        emojiTitle.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        emojiTitle.setGravity(Gravity.CENTER);
        emojiHeader.addView(emojiTitle, new LinearLayout.LayoutParams(0, s.pixels(40), 1));
        Button deleteEmoji = s.button(emojiHeader, "⌫", this::deleteFromEmojiPicker);
        ((KeyboardPressButton) deleteEmoji).setKeyboardRole(KeyboardKeyRole.GLYPH);
        deleteEmoji.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        deleteEmoji.setPadding(0, 0, 0, 0);
        deleteEmoji.setMinHeight(0);
        deleteEmoji.setMinimumHeight(0);
        deleteEmoji.setContentDescription("删除");
        deleteEmoji.setLayoutParams(new LinearLayout.LayoutParams(s.pixels(48), s.pixels(40)));
        s.emojiPanel.addView(emojiHeader);
        s.emojiTabs = new LinearLayout(s);
        s.emojiTabs.setOrientation(LinearLayout.HORIZONTAL);
        s.emojiTabs.setContentDescription("表情分类");
        s.emojiPanel.addView(s.emojiTabs, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(40)));
        s.emojiStatus = new TextView(s);
        s.emojiStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        s.emojiStatus.setGravity(Gravity.CENTER_VERTICAL);
        s.emojiStatus.setPadding(s.pixels(6), 0, s.pixels(6), 0);
        s.emojiPanel.addView(s.emojiStatus, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(24)));
        s.emojiGrid = new LinearLayout(s);
        s.emojiGrid.setOrientation(LinearLayout.VERTICAL);
        s.emojiGridScroll = new ScrollView(s);
        s.emojiGridScroll.setFillViewport(false);
        s.emojiGridScroll.setVerticalScrollBarEnabled(false);
        s.emojiGridScroll.setContentDescription("表情网格；每行八个");
        s.emojiGridScroll.addView(s.emojiGrid, new ScrollView.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.emojiGridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            if (scrollY > oldY && !view.canScrollVertically(1)) s.loadEmojiPage();
        });
        s.emojiPanel.addView(s.emojiGridScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        s.emojiPanel.setVisibility(View.GONE);
        s.keyboardSurface.addView(s.emojiPanel, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
    }
}
