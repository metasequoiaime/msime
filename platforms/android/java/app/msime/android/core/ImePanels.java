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
        // 分类栏只剩图标，分类名和数量放在网格的状态描述里，界面上不再占一行。
        String title = s.emojiSelectedCategory == -1 ? EmojiCatalogModel.RECENTS.title()
            : s.emojiSelectedCategory >= 0 && s.emojiSelectedCategory < EmojiCatalogModel.categories().size()
            ? EmojiCatalogModel.categories().get(s.emojiSelectedCategory).title() : "表情";
        if (s.emojiLoading && s.emojiItems.isEmpty()) showEmojiStatus(title + " · 正在加载…");
        else if (s.emojiItems.isEmpty()) showEmojiStatus(title + " · 暂无表情");
        else showEmojiStatus(title + " · " + s.emojiItems.size() + " 个表情");
    }

    /** 表情面板的状态文字写在网格的状态描述上（读屏读出、设备测试据此数数量）。 */
    void showEmojiStatus(String text) {
        if (s.emojiGridScroll != null && Build.VERSION.SDK_INT >= 30) s.emojiGridScroll.setStateDescription(text);
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
        KeyboardGeometry.setKeyTextSize(tab, 17);
        tab.setPadding(0, 0, 0, 0);
        tab.setMinWidth(0);
        tab.setMinimumWidth(0);
        tab.setMinHeight(0);
        tab.setMinimumHeight(0);
        tab.setIncludeFontPadding(false);
        tab.setSelected(s.emojiSelectedCategory == category);
        tab.setContentDescription("表情分类 " + entry.title());
        if (Build.VERSION.SDK_INT >= 30)
            tab.setStateDescription(tab.isSelected() ? "已选中" : "未选中");
        tab.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(tab);
            s.selectEmojiCategory(category);
        });
        s.emojiTabs.addView(tab, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
    }

    /** 共享换肤遍历之后再画底栏分类：选中的分类是键帽色药丸，其余只是半透明图标。 */
    void styleEmojiChrome() {
        if (s.emojiTabs == null) return;
        for (int index = 0; index < s.emojiTabs.getChildCount(); index++) {
            View tab = s.emojiTabs.getChildAt(index);
            if (tab.isSelected()) {
                GradientDrawable face = DrawablePolicy.rounded(
                    Color.parseColor(s.emojiSkin.keyBackground()), s.pixels(8));
                tab.setBackground(new InsetDrawable(face, s.pixels(2), s.pixels(3), s.pixels(2), s.pixels(3)));
                tab.setAlpha(1f);
            } else {
                tab.setBackground(null);
                tab.setAlpha(.6f);
            }
            tab.setElevation(0);
        }
    }

    void renderEmojiGrid() {
        if (s.emojiGrid == null) return;
        s.emojiGrid.removeAllViews();
        // 每行固定八等分，网格可见区放三行；不足一行时格子保持原宽，不会被拉满整行。
        int visible = s.emojiGridScroll == null ? 0 : s.emojiGridScroll.getHeight();
        int rowHeight = visible > 0 ? Math.max(s.pixels(40), visible / 3) : s.pixels(48);
        LinearLayout row = null;
        for (EmojiCatalogModel.Item item : s.emojiItems) {
            if (row == null || row.getChildCount() == EmojiCatalogModel.COLUMNS) {
                row = new LinearLayout(s);
                row.setWeightSum(EmojiCatalogModel.COLUMNS);
                s.emojiGrid.addView(row, new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, rowHeight));
            }
            Button cell = s.keyboardKey(item.text(), "表情 " + item.text(),
                () -> insertEmoji(item.text()));
            ((KeyboardPressButton) cell).setKeyboardRole(KeyboardKeyRole.PLAIN);
            KeyboardGeometry.setKeyTextSize(cell, 26);
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
        // 面板原先没有底色，网格空着时直接透出底下的字母键；铺上键盘底图。
        s.imeStyler.applySkinBackground(s.symbolPanel);
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
        pendingSkinKey = null;
        renderSkinPicker();
        s.skinScroll.setVisibility(View.VISIBLE);
    }

    /** 皮肤面板正在应用、尚未写回偏好的那一款；保存回来之前选中态按它画，点下去就换色。 */
    private String pendingSkinKey;

    void renderSkinPicker() {
        if (s.skinPanel == null) return;
        s.skinPanel.removeAllViews();
        s.skinPanel.setPadding(s.pixels(8), s.pixels(10), s.pixels(8), s.pixels(6));
        JSONObject preferences = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        boolean hostDark = KeyboardSkin.resolveDark(
            preferences == null ? "follow" : preferences.optString("screen_keyboard_theme", "follow"),
            preferences == null ? "system" : preferences.optString("theme", "system"), s.systemDark());
        // 目录里的全局主题按共享目录的顺序（含水杉四季与春夏秋冬），后面接「我的设计」。
        JSONObject customTheme = preferences == null ? null : preferences.optJSONObject("custom_theme");
        JSONArray themes = s.themeCatalog();
        java.util.List<MSIMEInputService.SkinChoice> choices =
            new java.util.ArrayList<>(themes.length());
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = entry.optString("id", "");
            if (id.isEmpty()) continue;
            String themeName = entry.optString("title", id);
            KeyboardSkin choice = "custom".equals(id) ? s.themeSkin(id, customTheme, hostDark)
                : KeyboardSkin.resolved(entry, themeName, hostDark, null);
            choices.add(new MSIMEInputService.SkinChoice(id, choice.title(), choice, null));
        }
        if (choices.isEmpty()) {
            KeyboardSkin system = KeyboardSkin.system(hostDark);
            choices.add(new MSIMEInputService.SkinChoice(system.id(), system.title(), system, null));
        }
        // 已获取的设计与社区目录缓存里的设计，按皮肤的绘制键去重；「我的皮肤」与其中某一款相同时只留带名字的那一格。
        java.util.Set<String> libraryIds = new java.util.HashSet<>();
        java.util.Set<String> namedKeys = new java.util.HashSet<>();
        try {
            for (CustomSkinLibrary.Item item : CustomSkinLibrary.read(java.nio.file.Paths.get(s.preferencesDirectory))) {
                JSONObject design = item.design();
                KeyboardSkin skin = KeyboardSkin.custom(design, hostDark);
                libraryIds.add(item.id());
                namedKeys.add(skin.key());
                choices.add(new MSIMEInputService.SkinChoice("custom", item.name(), skin, design));
            }
        } catch (Exception ignored) {
            // 写到一半的自定义库不能把主题也藏起来。
        }
        // 社区里还没获取的皮肤：目录由 App 缓存（键盘不为浏览目录联网），选中时先存进皮肤库再换上。
        java.util.Map<MSIMEInputService.SkinChoice, CommunitySkinCache.Entry> uninstalled = new java.util.HashMap<>();
        if (!s.preferencesDirectory.isEmpty()) {
            for (CommunitySkinCache.Entry entry : CommunitySkinCache.read(java.nio.file.Paths.get(s.preferencesDirectory))) {
                if (libraryIds.contains(entry.id())) continue;
                KeyboardSkin skin = KeyboardSkin.custom(entry.design(), hostDark);
                if (!namedKeys.add(skin.key())) continue;
                MSIMEInputService.SkinChoice choice = new MSIMEInputService.SkinChoice("custom", entry.name(), skin, entry.design());
                uninstalled.put(choice, entry);
                choices.add(choice);
            }
        }
        choices.removeIf(choice -> "custom".equals(choice.id()) && choice.design() == null
            && namedKeys.contains(choice.skin().key()));
        String globalTheme = preferences == null ? "system" : preferences.optString("global_theme", "system");
        PagedTileGrid grid = new PagedTileGrid(s);
        grid.setGrid(4, 2);
        // 行高容下按 390:292 的迷你键盘（约 86 dp 宽时 65 dp 高）、描边和下方的名字；行距、列距取设计的 6 / 10。
        grid.setSpacing(92, 6, 10, 4);
        grid.setContentDescription("键盘皮肤选择器");
        java.util.List<KeyboardSkinCard> cards = new java.util.ArrayList<>(choices.size());
        int selectedIndex = 0;
        for (int index = 0; index < choices.size(); index++) {
            MSIMEInputService.SkinChoice choice = choices.get(index);
            KeyboardSkinCard card = new KeyboardSkinCard(s, choice.skin(), choice.title());
            String key = choiceKey(choice);
            boolean selected = pendingSkinKey != null ? pendingSkinKey.equals(key)
                : choice.design() == null ? choice.id().equals(globalTheme)
                : "custom".equals(globalTheme) && s.skin.key().equals(choice.skin().key());
            card.setSelected(selected);
            if (selected) selectedIndex = index;
            if ("system".equals(choice.id())) {
                card.setSplitPreview(Color.parseColor(KeyboardSkin.system(false).background()),
                    Color.parseColor(KeyboardSkin.system(true).background()));
            }
            card.setContentDescription("屏幕键盘皮肤 " + choice.title());
            if (Build.VERSION.SDK_INT >= 30)
                card.setStateDescription(selected ? "已选中" : "未选中");
            card.setOnClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(card);
                pendingSkinKey = key;
                for (KeyboardSkinCard other : cards) {
                    other.setSelected(other == card);
                    if (Build.VERSION.SDK_INT >= 30) other.setStateDescription(other == card ? "已选中" : "未选中");
                }
                // 选中即换色并收起面板回到键盘，换上的皮肤直接在键盘上看。还没获取的社区皮肤先在同一个偏好线程上存进皮肤库，排在保存选择之前。
                CommunitySkinCache.Entry install = uninstalled.get(choice);
                if (install != null) {
                    final String directory = s.preferencesDirectory;
                    s.preferencesWorker.execute(() -> {
                        try {
                            if (!CustomSkinLibrary.add(java.nio.file.Paths.get(directory), install.id(), install.name(), install.design()))
                                android.util.Log.i("MSIMESkin", "Skin library is full; the community skin is applied without being saved");
                        } catch (java.io.IOException | RuntimeException error) {
                            android.util.Log.w("MSIMESkin", "Community skin was not saved to the library", error);
                        }
                    });
                }
                s.saveKeyboardSkin(choice.id(), choice.design());
                s.recordSkinStatistics(choice.design() == null ? choice.id() : "custom");
                styleSkinPicker(cards);
                s.closeSkinPicker();
                s.render();
            });
            cards.add(card);
            grid.addView(card);
        }
        addPagedGrid(s.skinPanel, grid, PagedTileGrid.pageOf(selectedIndex, PagedTileGrid.perPage(4, 2)));
        s.imeStyler.applySkin();
        styleSkinPicker(cards);
    }

    private static String choiceKey(MSIMEInputService.SkinChoice choice) {
        return choice.design() == null ? "theme:" + choice.id() : "design:" + choice.design().toString().hashCode();
    }

    /** 皮肤保存回来（成功或失败）后清掉临时选中态，下次按偏好画。 */
    void finishSkinPick() {
        pendingSkinKey = null;
        if (s.skinScroll != null && s.skinScroll.getVisibility() == View.VISIBLE) renderSkinPicker();
    }

    private void styleSkinPicker(java.util.List<KeyboardSkinCard> cards) {
        int accent = Color.parseColor(s.skin.accent());
        int hairline = Color.parseColor(s.skin.hairline());
        int label = Color.parseColor(s.skin.keyForeground());
        for (KeyboardSkinCard card : cards) card.setTileColors(accent, hairline, label);
        styleDots(s.skinPanel);
    }

    /** 分页网格加下方页点：皮肤面板与输入方式面板共用。 */
    private void addPagedGrid(LinearLayout parent, PagedTileGrid grid, int initialPage) {
        parent.addView(grid, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        parent.addView(new View(s), new LinearLayout.LayoutParams(0, 0, 1));
        KeyboardPagerDots dots = new KeyboardPagerDots(s);
        dots.setTag(PAGER_DOTS_TAG);
        dots.setCount(grid.pageCount());
        dots.setActive(initialPage, false);
        dots.setVisibility(grid.pageCount() > 1 ? View.VISIBLE : View.INVISIBLE);
        LinearLayout.LayoutParams dotParams = new LinearLayout.LayoutParams(
            s.pixels(Math.round(KeyboardPagerDots.totalWidthDp(Math.max(1, grid.pageCount())))), s.pixels(10));
        dotParams.gravity = Gravity.CENTER_HORIZONTAL;
        dotParams.topMargin = s.pixels(6);
        dotParams.bottomMargin = s.pixels(6);
        parent.addView(dots, dotParams);
        grid.setOnPageChangeListener((page, count) -> dots.setActive(page, true));
        grid.post(() -> grid.setPage(initialPage, false));
    }

    private static final String PAGER_DOTS_TAG = "msime-pager-dots";

    private void styleDots(LinearLayout parent) {
        View dots = parent == null ? null : parent.findViewWithTag(PAGER_DOTS_TAG);
        if (dots instanceof KeyboardPagerDots pager)
            pager.setColors(Color.parseColor(s.skin.accent()), Color.parseColor(s.skin.hairline()));
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
        KeyboardGeometry.setKeyTextSize(s.replySourceButton, 15);
        s.replySourceButton.setContentDescription("回复源文字");
        compactReplyControl(s.replySourceButton, 0);
        s.replySourceButton.setLayoutParams(new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        s.replyPasteButton = MSIMEInputService.role(s.button(s.replySourceCard, "粘贴", this::pasteReplySource),
            KeyboardKeyRole.PLAIN);
        s.replyPasteButton.setContentDescription("粘贴回复源文字");
        KeyboardGeometry.setKeyTextSize(s.replyPasteButton, 13);
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
        KeyboardGeometry.setKeyTextSize(s.replyStatus, 11);
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
        KeyboardGeometry.setKeyTextSize(s.replyStyleResetButton, 12);
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
        KeyboardGeometry.setKeyTextSize(segment, 14);
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
        KeyboardGeometry.setKeyTextSize(button, 14);
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
                KeyboardGeometry.setKeyTextSize(candidate, 15);
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
        return DrawablePolicy.rounded(color, radius);
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
                Math.max(0f, radius - s.pixels(2))) : null);
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

    /** AI 润色面板：与回复面板同一套样式，标题行（返回键盘）、目标与模型一行小字、键帽色圆角卡里放待润色或润色后的文字，底部一颗强调色主操作。 */
    void renderAiPolish() {
        if (s.aiPolishPanel == null || s.aiPolishActions == null) return;
        s.aiPolishPanel.removeAllViews();
        s.aiPolishActions.removeAllViews();
        s.aiPolishPanel.setPadding(s.pixels(10), s.pixels(6), s.pixels(10), s.pixels(6));
        s.aiPolishActions.setPadding(s.pixels(10), 0, s.pixels(10), s.pixels(8));
        LinearLayout header = new LinearLayout(s);
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = new TextView(s);
        title.setText(s.aiOutputText.isEmpty() ? "AI 润色" : "润色结果");
        KeyboardGeometry.setKeyTextSize(title, 15);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button back = MSIMEInputService.role(s.button(header, "返回键盘", s::closeAiPolish), KeyboardKeyRole.GLYPH);
        KeyboardGeometry.setKeyTextSize(back, 13);
        compactReplyControl(back, s.pixels(8));
        back.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, s.pixels(32)));
        s.aiPolishPanel.addView(header);
        java.util.List<TextView> secondary = new java.util.ArrayList<>();
        TextView error = null;
        if (!s.aiError.isEmpty()) {
            error = new TextView(s);
            error.setText(s.aiError);
            KeyboardGeometry.setKeyTextSize(error, 13);
            error.setContentDescription("AI 润色状态");
            s.aiPolishPanel.addView(error);
        }
        if (s.aiRequestConfiguration != null) {
            TextView destination = new TextView(s);
            destination.setText("发送到 " + s.aiRequestConfiguration.destination() + " · "
                + s.aiRequestConfiguration.model());
            KeyboardGeometry.setKeyTextSize(destination, 12);
            destination.setContentDescription("AI 请求目标和模型");
            s.aiPolishPanel.addView(destination);
            secondary.add(destination);
        }
        TextView label = new TextView(s);
        label.setText(s.aiOutputText.isEmpty() ? "待发送的选中文字" : "润色结果");
        KeyboardGeometry.setKeyTextSize(label, 12);
        label.setPadding(0, s.pixels(6), 0, s.pixels(4));
        s.aiPolishPanel.addView(label);
        secondary.add(label);
        TextView content = new TextView(s);
        content.setText(s.aiOutputText.isEmpty() ? s.aiSourceText : s.aiOutputText);
        KeyboardGeometry.setKeyTextSize(content, 15);
        content.setPadding(s.pixels(12), s.pixels(10), s.pixels(12), s.pixels(10));
        content.setContentDescription(s.aiOutputText.isEmpty() ? "待润色文字" : "AI 润色结果");
        s.aiPolishPanel.addView(content, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        Button primary;
        if (s.aiBusy) {
            TextView progress = new TextView(s);
            progress.setText("正在请求…");
            KeyboardGeometry.setKeyTextSize(progress, 12);
            progress.setPadding(0, s.pixels(6), 0, 0);
            s.aiPolishPanel.addView(progress);
            secondary.add(progress);
            primary = s.button(s.aiPolishActions, "取消请求", () -> {
                s.cancelAiRequest();
                renderAiPolish();
            });
        } else if (s.aiOutputText.isEmpty()) {
            primary = s.button(s.aiPolishActions, "发送选中文字", this::sendAiPolish);
            primary.setEnabled(s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
        } else {
            primary = s.button(s.aiPolishActions, "替换选中文字", this::replaceAiSelection);
            primary.setEnabled(s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
        }
        compactReplyControl(primary, 0);
        KeyboardGeometry.setKeyTextSize(primary, 15);
        primary.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(44)));
        s.imeStyler.applySkin();
        // 换肤遍历之后补上卡片底色、次要字色和主操作的强调色。
        float radius = s.pixels(10);
        content.setBackground(replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        for (TextView text : secondary) text.setTextColor(ImeStyler.fade(s.skin.keyForeground(), .6));
        if (error != null) error.setTextColor(Color.parseColor(s.skin.accent()));
        boolean busy = s.aiBusy;
        primary.setBackground(replySurface(busy ? ImeStyler.fade(s.skin.keyBackground(), .7)
            : Color.parseColor(s.skin.accent()), radius));
        primary.setTextColor(busy ? Color.parseColor(s.skin.keyForeground()) : Color.parseColor(s.skin.onAccent()));
        primary.setAlpha(primary.isEnabled() ? 1f : .45f);
        primary.setElevation(0);
    }

    void showSchemePicker() {
        // 其他设置正在保存时只是这一下不打开：保存一瞬间就完成，不必弹「尚未就绪」。
        if (s.touchGeometrySaving || s.traditionalOutputSaving) return;
        if (s.session == 0 || s.preferencesSnapshot == null || s.preferencesDirectory.isEmpty()) {
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
        s.schemePanel.setPadding(s.pixels(8), s.pixels(14), s.pixels(8), s.pixels(6));
        // 4×2 分页网格：已启用的方案按共享目录的顺序，英文 26 键排在第三格（方案不够时排最后），末尾是「+ 添加语言」。
        java.util.List<KeyboardScheme> schemes = s.visibleSchemes;
        final int englishIndex = Math.min(2, schemes.size());
        int cardCount = schemes.size() + 2;
        PagedTileGrid grid = new PagedTileGrid(s);
        grid.setGrid(4, 2);
        // 卡片高 60 dp、行距 12 dp：两行总高与原先的 56 + 16 相同，但字形区（44 dp）下面的方案名在 1.15 倍字体下也放得下，不再被切掉下半截。
        grid.setSpacing(60, 12, 4, 4);
        grid.setContentDescription("输入方案卡片区域");
        java.util.List<KeyboardSchemeCard> schemeCards = new java.util.ArrayList<>();
        java.util.List<Boolean> cardSelection = new java.util.ArrayList<>();
        int selectedIndex = 0;
        for (int index = 0; index < cardCount; index++) {
            final KeyboardSchemeCard card;
            final boolean selected;
            final String title;
            if (index == cardCount - 1) {
                title = "添加语言";
                selected = false;
                card = new KeyboardSchemeCard(s, "+", "", "添加语言");
                card.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    android.os.Bundle args = new android.os.Bundle();
                    args.putBoolean("add_language", true);
                    s.closeSchemePicker();
                    s.openHostPage("TYPING", args);
                });
            } else if (index == englishIndex) {
                // 英文是平台的文字模式，不是第二个持久化的 Engine 方案。
                title = "英文 26 键";
                selected = s.dedicatedEnglish;
                card = new KeyboardSchemeCard(s, "EN", "26", title);
                card.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    s.selectEnglishScheme();
                });
                card.setEnabled(!s.schemeSaving);
            } else {
                KeyboardScheme scheme = schemes.get(index > englishIndex ? index - 1 : index);
                title = scheme.title(s.wubiProfile);
                selected = !s.dedicatedEnglish && scheme == s.selectedScheme;
                card = new KeyboardSchemeCard(s, scheme.glyph(), scheme.badge(s.wubiProfile), title);
                card.setOnClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    s.selectKeyboardScheme(scheme);
                });
                card.setEnabled(!s.schemeSaving);
            }
            card.setContentDescription("输入方案卡片 " + title);
            if (Build.VERSION.SDK_INT >= 30)
                card.setStateDescription(selected ? "已选中" : "未选中");
            if (selected) selectedIndex = index;
            schemeCards.add(card);
            cardSelection.add(selected);
            grid.addView(card);
        }
        addPagedGrid(s.schemePanel, grid, PagedTileGrid.pageOf(selectedIndex, PagedTileGrid.perPage(4, 2)));
        s.imeStyler.applySkin();
        // 必须在换肤遍历之后：那一趟会把每个 TextView 重新刷成 keyForeground。
        int accent = Color.parseColor(s.skin.accent());
        int foreground = Color.parseColor(s.skin.keyForeground());
        int panel = Color.parseColor(s.skin.background());
        for (int index = 0; index < schemeCards.size(); index++)
            schemeCards.get(index).paintTile(accent, foreground, panel, cardSelection.get(index));
        styleDots(s.schemePanel);
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
        // 输入框本身的限制之外还要过隐私闸门：隐私模式开着时同样不碰云剪贴板。
        return CloudClipboardPanelPolicy.cloudAllowed(s.editorInputType, s.allowLearning)
            && s.imePrivacyGate.pushesCloudClipboard();
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
        s.clipboardPanel.setPadding(s.pixels(8), s.pixels(8), s.pixels(8), s.pixels(8));
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!cloudAllowed) s.clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        boolean cloud = s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD;
        java.util.List<TextView> notes = new java.util.ArrayList<>(1);
        // 顶部一行小号操作：本机 / 云端分段（云端可用时）、刷新或清空；返回由工具栏的「返回键盘」负责。
        LinearLayout header = new LinearLayout(s);
        header.setGravity(Gravity.CENTER_VERTICAL);
        if (cloudAllowed) {
            addClipboardTab(header, CloudClipboardPanelPolicy.TAB_LOCAL, CloudClipboardPanelPolicy.Tab.LOCAL);
            addClipboardTab(header, CloudClipboardPanelPolicy.TAB_CLOUD, CloudClipboardPanelPolicy.Tab.CLOUD);
        }
        header.addView(new View(s), new LinearLayout.LayoutParams(0, 0, 1));
        if (cloud) {
            Button refresh = clipboardAction(header, "刷新", this::refreshCloudClipboard);
            refresh.setEnabled(s.cloudClipboardStatus != CloudClipboardPanelPolicy.Status.LOADING);
            refresh.setContentDescription("刷新云剪贴板");
        } else if (s.clipboardHistoryEnabled) {
            Button capture = clipboardAction(header, "保存当前", s::captureClipboardText);
            capture.setContentDescription("保存当前剪贴板文本");
            clipboardAction(header, "清空", s::confirmClearClipboardHistory);
        }
        s.clipboardPanel.addView(header, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(32)));
        if (cloud) {
            renderCloudClipboard(notes);
        } else if (!s.clipboardHistoryEnabled) {
            notes.add(clipboardNote("剪贴板历史未开启，可在设置中开启"));
        } else {
            try {
                java.util.List<ClipboardHistory.Item> items = s.clipboardHistory.load();
                if (items.isEmpty()) notes.add(clipboardNote("暂无历史 · 保存后点按插入 · 记录仅保存在本机"));
                long now = System.currentTimeMillis();
                for (ClipboardHistory.Item item : items) {
                    String meta = (item.pinned() ? "已置顶 · " : "") + "本机 · " + relativeTime(item.timestamp(), now);
                    Button card = clipboardCard(item.text(), meta, () -> s.insertClipboardText(item.text()));
                    card.setContentDescription((item.pinned() ? "已置顶；" : "") + "点按插入剪贴板记录，长按管理");
                    card.setOnLongClickListener(ignored -> {
                        s.manageClipboardItem(card, item);
                        return true;
                    });
                }
            } catch (IllegalStateException error) {
                notes.add(clipboardNote("历史记录无法读取，请清空后重试"));
            }
        }
        s.imeStyler.applySkin();
        for (TextView note : notes) note.setTextColor(ImeStyler.fade(s.skin.keyForeground(), .6));
    }

    private Button clipboardAction(LinearLayout header, String label, Runnable action) {
        Button button = MSIMEInputService.role(s.button(header, label, action), KeyboardKeyRole.GLYPH);
        KeyboardGeometry.setKeyTextSize(button, 13);
        compactReplyControl(button, s.pixels(10));
        button.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        return button;
    }

    private TextView clipboardNote(String text) {
        TextView note = new TextView(s);
        note.setText(text);
        KeyboardGeometry.setKeyTextSize(note, 13);
        note.setGravity(Gravity.CENTER);
        note.setPadding(s.pixels(12), s.pixels(20), s.pixels(12), s.pixels(20));
        s.clipboardPanel.addView(note, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        return note;
    }

    /** 剪贴板卡片：键帽色圆角卡，第一行是文字（最多两行），第二行是『已置顶 · 设备 · 时间』，元信息用次要色的小号字（经 span，换肤遍历刷字色时不受影响）。 */
    private Button clipboardCard(String text, String meta, Runnable action) {
        KeyboardPressButton card = new KeyboardPressButton(s);
        card.setKeyboardRole(KeyboardKeyRole.KEY);
        card.setAllCaps(false);
        android.text.SpannableStringBuilder label = new android.text.SpannableStringBuilder(text);
        if (!meta.isEmpty()) {
            label.append('\n');
            int start = label.length();
            label.append(meta);
            label.setSpan(new android.text.style.RelativeSizeSpan(.8f), start, label.length(),
                android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            label.setSpan(new android.text.style.ForegroundColorSpan(Color.parseColor(s.skin.secondary())),
                start, label.length(), android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        card.setText(label);
        KeyboardGeometry.setKeyTextSize(card, 15);
        card.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
        card.setMaxLines(3);
        card.setEllipsize(android.text.TextUtils.TruncateAt.END);
        card.setPadding(s.pixels(12), s.pixels(8), s.pixels(12), s.pixels(8));
        card.setMinHeight(0);
        card.setMinimumHeight(0);
        card.setStateListAnimator(null);
        card.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(card);
            action.run();
        });
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.topMargin = s.pixels(6);
        s.clipboardPanel.addView(card, params);
        return card;
    }

    /** 「刚刚 / N 分钟前 / N 小时前 / N 天前」；时间戳早于 2001 年的按秒解读。 */
    static String relativeTime(long timestamp, long now) {
        if (timestamp <= 0) return "";
        long millis = timestamp < 100_000_000_000L ? timestamp * 1000 : timestamp;
        long minutes = BoundsPolicy.nonNegative(now - millis) / 60_000;
        if (minutes < 1) return "刚刚";
        if (minutes < 60) return minutes + " 分钟前";
        if (minutes < 60 * 24) return (minutes / 60) + " 小时前";
        return (minutes / (60 * 24)) + " 天前";
    }

    /** 云端条目的更新时间（ISO-8601）换成相对时间；解析不了就不显示。 */
    static String relativeTime(String iso, long now) {
        if (iso == null || iso.isEmpty()) return "";
        try {
            return relativeTime(java.time.Instant.parse(iso).toEpochMilli(), now);
        } catch (java.time.format.DateTimeParseException error) {
            return "";
        }
    }

    void addClipboardTab(LinearLayout tabs, String title, CloudClipboardPanelPolicy.Tab tab) {
        Button button = clipboardAction(tabs, title, () -> selectClipboardTab(tab));
        button.setSelected(s.clipboardTab == tab);
        button.setContentDescription("剪贴板分类 " + title);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(button.isSelected() ? "已选中" : "未选中");
        button.setAlpha(button.isSelected() ? 1f : .55f);
    }

    void renderCloudClipboard(java.util.List<TextView> notes) {
        String message = CloudClipboardPanelPolicy.message(s.cloudClipboardStatus, s.cloudClipboardItems.size());
        if (!CloudClipboardPanelPolicy.showsItems(s.cloudClipboardStatus) || s.cloudClipboardItems.isEmpty()) {
            notes.add(clipboardNote(message));
            return;
        }
        long now = System.currentTimeMillis();
        for (BackendAccount.ClipboardItem item : s.cloudClipboardItems) {
            Button card = clipboardCard(item.text(), "云端 · " + relativeTime(item.updatedAt(), now),
                () -> insertCloudClipboardText(item.text()));
            card.setContentDescription("点按插入云剪贴板记录");
        }
    }

    /** 常用语面板的读取代次：面板关掉或重开后，迟到的结果直接丢弃。 */
    private long phraseGeneration;
    /** 没有常用语时的提示；它下面跟一个直达应用常用语页的「添加常用语」按钮。 */
    private static final String EMPTY_PHRASES = "还没有常用语";

    /** 工具栏「常用语」：先完成当前组词，在工作线程经 {@link CommonPhrasesStore} 读无编码常用语，全宽列表，点一条上屏并关闭面板。 */
    void showCommonPhrases() {
        if (s.phraseScroll == null || s.phrasePanel == null) return;
        s.command(2);
        s.closeCandidatePanel();
        long generation = ++phraseGeneration;
        renderCommonPhrases(java.util.List.of(), "正在读取常用语…");
        s.phraseScroll.setVisibility(View.VISIBLE);
        Runnable load = () -> {
            CommonPhrasesStore.Result result;
            try {
                result = CommonPhrasesStore.load(s);
            } catch (RuntimeException | LinkageError error) {
                result = null;
            }
            java.util.List<String> phrases = new java.util.ArrayList<>();
            String message;
            if (result == null || !result.ok()) {
                message = result == null || result.failure().isEmpty() ? "常用语读取失败" : result.failure();
            } else {
                for (CommonPhrasesStore.Phrase phrase : result.document().phrases())
                    if (!phrase.text().isEmpty()) phrases.add(phrase.text());
                message = phrases.isEmpty() ? EMPTY_PHRASES : null;
            }
            final String note = message;
            s.main.post(() -> {
                if (generation != phraseGeneration || s.phraseScroll == null
                        || s.phraseScroll.getVisibility() != View.VISIBLE) return;
                renderCommonPhrases(phrases, note);
            });
        };
        try {
            s.preferencesWorker.execute(load);
        } catch (RuntimeException error) {
            renderCommonPhrases(java.util.List.of(), "常用语读取失败");
        }
    }

    private void renderCommonPhrases(java.util.List<String> phrases, String message) {
        LinearLayout panel = s.phrasePanel;
        panel.removeAllViews();
        panel.setPadding(s.pixels(8), s.pixels(4), s.pixels(8), s.pixels(8));
        s.imeStyler.applySkinBackground(panel);
        TextView note = null;
        if (message != null) {
            note = new TextView(s);
            note.setText(message);
            KeyboardGeometry.setKeyTextSize(note, 14);
            note.setGravity(Gravity.CENTER);
            note.setPadding(s.pixels(12), s.pixels(24), s.pixels(12), s.pixels(24));
            panel.addView(note, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        if (EMPTY_PHRASES.equals(message)) {
            // 空的时候给一条去处：直接打开应用的常用语页去添加，而不是让人自己退出键盘去找。
            KeyboardPressButton add = new KeyboardPressButton(s);
            add.setKeyboardRole(KeyboardKeyRole.RETURN);
            add.setAllCaps(false);
            add.setText("添加常用语");
            KeyboardGeometry.setKeyTextSize(add, 15);
            add.setMinHeight(s.pixels(44));
            add.setMinimumHeight(s.pixels(44));
            add.setPadding(s.pixels(24), 0, s.pixels(24), 0);
            add.setStateListAnimator(null);
            add.setContentDescription("添加常用语");
            add.setOnClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(add);
                s.closeCommonPhrases();
                s.openHostPage("PHRASES");
            });
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
            params.gravity = Gravity.CENTER_HORIZONTAL;
            panel.addView(add, params);
        }
        java.util.List<View> lines = new java.util.ArrayList<>();
        for (String phrase : phrases) {
            KeyboardPressButton row = new KeyboardPressButton(s);
            row.setKeyboardRole(KeyboardKeyRole.PLAIN);
            row.setAllCaps(false);
            row.setText(phrase);
            KeyboardGeometry.setKeyTextSize(row, 15);
            row.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
            row.setMaxLines(2);
            row.setEllipsize(android.text.TextUtils.TruncateAt.END);
            row.setMinHeight(s.pixels(44));
            row.setMinimumHeight(s.pixels(44));
            row.setPadding(s.pixels(12), s.pixels(8), s.pixels(12), s.pixels(8));
            row.setStateListAnimator(null);
            row.setContentDescription("常用语 " + (phrase.length() > 20 ? phrase.substring(0, 20) : phrase));
            row.setOnClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(row);
                if (s.connection == null) return;
                s.command(2);
                s.commitText(phrase);
                s.closeCommonPhrases();
                s.render();
            });
            panel.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            View hairline = new View(s);
            LinearLayout.LayoutParams line = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, Math.max(1, s.pixels(1)));
            line.setMarginStart(s.pixels(12));
            line.setMarginEnd(s.pixels(12));
            panel.addView(hairline, line);
            lines.add(hairline);
        }
        s.imeStyler.applySkin();
        for (View hairline : lines) hairline.setBackgroundColor(Color.parseColor(s.skin.hairline()));
        if (note != null) note.setTextColor(ImeStyler.fade(s.skin.keyForeground(), .6));
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
        // 设计：盖在键区上、不盖顶部一行；上面是每行八个的表情网格（可见三行，可滚动），底栏是 ABC | 分类 | ⌫。高度由 PanelSurface 限定为键区高度。
        s.emojiPanel = new LinearLayout(s);
        s.emojiPanel.setOrientation(LinearLayout.VERTICAL);
        s.emojiPanel.setPadding(s.pixels(6), s.pixels(4), s.pixels(6), s.pixels(4));
        s.emojiPanel.setBackgroundColor(Color.parseColor(s.skin.background()));
        s.emojiPanel.setContentDescription("表情面板");
        s.emojiPanel.setFocusable(true);
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
        // 第一次排布出真实高度后按三行重排格子高度。
        s.emojiGridScroll.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            if (bottom - top != oldBottom - oldTop && s.emojiPickerVisible()) view.post(this::renderEmojiGrid);
        });
        s.emojiPanel.addView(s.emojiGridScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        LinearLayout bar = new LinearLayout(s);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        Button abc = MSIMEInputService.role(s.button(bar, "ABC", s::closeEmojiPicker), KeyboardKeyRole.ACCENT);
        KeyboardGeometry.setKeyTextSize(abc, 14);
        compactReplyControl(abc, 0);
        abc.setContentDescription("返回键盘");
        abc.setLayoutParams(new LinearLayout.LayoutParams(s.pixels(60), s.pixels(40)));
        s.emojiTabs = new LinearLayout(s);
        s.emojiTabs.setOrientation(LinearLayout.HORIZONTAL);
        s.emojiTabs.setContentDescription("表情分类");
        LinearLayout.LayoutParams tabsParams = new LinearLayout.LayoutParams(0, s.pixels(40), 1);
        tabsParams.setMarginStart(s.pixels(6));
        tabsParams.setMarginEnd(s.pixels(6));
        bar.addView(s.emojiTabs, tabsParams);
        Button deleteEmoji = MSIMEInputService.role(s.button(bar, "⌫", this::deleteFromEmojiPicker),
            KeyboardKeyRole.ACCENT);
        KeyboardGeometry.setKeyTextSize(deleteEmoji, 18);
        compactReplyControl(deleteEmoji, 0);
        deleteEmoji.setContentDescription("删除");
        deleteEmoji.setLayoutParams(new LinearLayout.LayoutParams(s.pixels(60), s.pixels(40)));
        s.emojiPanel.addView(bar, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(46)));
        s.emojiPanel.setVisibility(View.GONE);
        s.keyboardSurface.addView(s.emojiPanel, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
    }
}
