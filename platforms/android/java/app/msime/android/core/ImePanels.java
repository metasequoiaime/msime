package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.SharedPreferences;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.InsetDrawable;
import android.os.Build;
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
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 盖在键盘上的各个面板：表情、符号、皮肤、输入方案、剪贴板、回复键盘、AI 润色、本地输入菜单与更多工具的入口；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImePanels {
    private static final String SYMBOL_RECENTS_PREFERENCES = "android-symbol-recents";
    private static final String SYMBOL_RECENTS_KEY = "items";
    private final MSIMEInputService s;
    private SharedPreferences symbolPreferences;
    private int replyReadGeneration;
    private int replyMenuReadGeneration;

    ImePanels(MSIMEInputService s) {
        this.s = s;
    }

    void showLocalInputMenu() {
        if (s.preedit == null || !s.supportsLocalTools() || s.view == null
                || !InputViewValuePolicy.editingText(s.view).isEmpty()
                || !"none".equals(JsonPolicy.strictStringOrEmpty(s.view.opt("local_mode")))) return;
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
        KeyboardPressButton tab = emojiButton(KeyboardKeyRole.PLAIN, false,
            () -> s.selectEmojiCategory(category));
        ViewPolicy.setTextSizeLabel(tab, entry.icon(), 17);
        ViewPolicy.clearFontPadding(tab);
        ViewPolicy.setSelected(tab, s.emojiSelectedCategory == category);
        tab.setContentDescription("表情分类 " + entry.title());
        if (Build.VERSION.SDK_INT >= 30)
            tab.setStateDescription(tab.isSelected() ? "已选中" : "未选中");
        s.emojiTabs.addView(tab, KeyboardGeometry.weightedMatchParentParams(1));
    }

    private static void compactEmojiButton(Button button) {
        ViewPolicy.clearPadding(button);
        ViewPolicy.clearMinimumSize(button);
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
                ViewPolicy.setActiveAlpha(tab, true, .6f);
            } else {
                ViewPolicy.clearBackground(tab);
                ViewPolicy.setActiveAlpha(tab, false, .6f);
            }
            ViewPolicy.clearElevation(tab);
        }
    }

    void renderEmojiGrid() {
        if (s.emojiGrid == null) return;
        s.emojiGrid.removeAllViews();
        // 每行固定八等分，网格可见区放三行；不足一行时格子保持原宽，不会被拉满整行。
        int visible = s.emojiGridScroll == null ? 0 : s.emojiGridScroll.getHeight();
        int rowHeight = visible > 0
            ? BoundsPolicy.atLeast(s.pixels(40), visible / 3) : s.pixels(48);
        LinearLayout row = null;
        for (EmojiCatalogModel.Item item : s.emojiItems) {
            if (row == null || row.getChildCount() == EmojiCatalogModel.COLUMNS) {
                row = KeyboardGeometry.row(s);
                row.setWeightSum(EmojiCatalogModel.COLUMNS);
                s.emojiGrid.addView(row, KeyboardGeometry.matchWidthHeightPx(rowHeight));
            }
            Button cell = emojiCell(item);
            row.addView(cell, KeyboardGeometry.weightedMatchParentParams(1));
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

    private Button emojiCell(EmojiCatalogModel.Item item) {
        KeyboardPressButton cell = emojiButton(KeyboardKeyRole.PLAIN, true,
            () -> insertEmoji(item.text()));
        cell.setText(item.text());
        cell.setContentDescription("按键 表情 " + item.text());
        ViewPolicy.setTextSizeSp(cell, 26);
        return cell;
    }

    private KeyboardPressButton emojiButton(KeyboardKeyRole role, boolean counted, Runnable action) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        button.setKeyboardRole(role);
        compactEmojiButton(button);
        if (counted) {
            s.imeStyler.styleButton(button, false);
            s.bindCountedAction(button, action);
        } else bindFeedbackAction(button, action);
        return button;
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
        ViewPolicy.show(s.emojiPanel);
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
        s.symbolPanel.resetForPresentation(loadSymbolRecents());
        // 面板原先没有底色，网格空着时直接透出底下的字母键；铺上键盘底图。
        s.imeStyler.applySkinBackground(s.symbolPanel);
        ViewPolicy.show(s.symbolPanel);
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
        int generation = ++replyReadGeneration;
        if (style != null && style.startsWith("community:")) {
            final CommunityReplyLibrary library = s.communityReplyLibrary;
            s.preferencesWorker.execute(() -> {
                final java.util.List<CommunityReplyLibrary.Template> templates;
                try { templates = library == null ? java.util.List.of() : library.read(); }
                catch (java.io.IOException error) {
                    s.main.post(() -> {
                        if (generation == replyReadGeneration) {
                            s.replyModel.showStatus("回复模板无法读取，请重试");
                            renderReplyKeyboard();
                        }
                    });
                    return;
                }
                s.main.post(() -> {
                    if (generation == replyReadGeneration) generateReplyLoaded(style, templates);
                });
            });
            return;
        }
        generateReplyLoaded(style, java.util.List.of());
    }

    private void generateReplyLoaded(String style, java.util.List<CommunityReplyLibrary.Template> templates) {
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
        int generation = ++replyMenuReadGeneration;
        final CommunityReplyLibrary library = s.communityReplyLibrary;
        s.preferencesWorker.execute(() -> {
            final java.util.List<CommunityReplyLibrary.Template> templates;
            try { templates = library == null ? java.util.List.of() : library.read(); }
            catch (java.io.IOException error) {
                s.main.post(() -> {
                    if (generation == replyMenuReadGeneration) {
                        s.replyModel.showStatus("回复模板无法读取，请重试");
                        renderReplyKeyboard();
                    }
                });
                return;
            }
            s.main.post(() -> {
                if (generation == replyMenuReadGeneration) showReplyTemplatesLoaded(templates);
            });
        });
    }

    private void showReplyTemplatesLoaded(java.util.List<CommunityReplyLibrary.Template> templates) {
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
        ViewPolicy.show(s.skinScroll);
    }

    /** 皮肤面板正在应用、尚未写回偏好的那一款；保存回来之前选中态按它画，点下去就换色。 */
    private String pendingSkinKey;
    private int skinRenderGeneration;

    void renderSkinPicker() {
        if (s.skinPanel == null) return;
        JSONObject preferences = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        boolean hostDark = KeyboardSkin.resolveDark(
            InputViewValuePolicy.textOr(preferences, "screen_keyboard_theme", "follow"),
            InputViewValuePolicy.textOr(preferences, "theme", "system"), s.systemDark());
        JSONArray themes = s.themeCatalog();
        int generation = ++skinRenderGeneration;
        renderSkinPickerLoaded(preferences, hostDark, themes, java.util.List.of(), java.util.List.of());
        String directory = s.preferencesDirectory;
        s.preferencesWorker.execute(() -> {
            java.util.List<CustomSkinLibrary.Item> library = java.util.List.of();
            java.util.List<CommunitySkinCache.Entry> community = java.util.List.of();
            if (!directory.isEmpty()) {
                java.nio.file.Path root = java.nio.file.Paths.get(directory);
                try { library = CustomSkinLibrary.read(root); }
                catch (Exception ignored) { }
                community = CommunitySkinCache.read(root);
            }
            java.util.List<CustomSkinLibrary.Item> loadedLibrary = library;
            java.util.List<CommunitySkinCache.Entry> loadedCommunity = community;
            s.main.post(() -> {
                if (generation != skinRenderGeneration || s.skinPanel == null) return;
                renderSkinPickerLoaded(preferences, hostDark, themes, loadedLibrary, loadedCommunity);
            });
        });
    }

    private void renderSkinPickerLoaded(JSONObject preferences, boolean hostDark, JSONArray themes,
            java.util.List<CustomSkinLibrary.Item> libraryItems,
            java.util.List<CommunitySkinCache.Entry> communityItems) {
        if (s.skinPanel == null) return;
        s.skinPanel.removeAllViews();
        KeyboardGeometry.setPaddingDp(s.skinPanel, s, 8, 10, 8, 6);
        // 目录里的全局主题按共享目录的顺序（含水杉四季与春夏秋冬），后面接「我的设计」。
        JSONObject customTheme = preferences == null ? null : preferences.optJSONObject("custom_theme");
        java.util.List<MSIMEInputService.SkinChoice> choices =
            new java.util.ArrayList<>(themes.length());
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = InputViewValuePolicy.textOr(entry, "id", "");
            if (id.isEmpty()) continue;
            String themeName = InputViewValuePolicy.textOr(entry, "title", id);
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
        for (CustomSkinLibrary.Item item : libraryItems) {
            JSONObject design = item.design();
            KeyboardSkin skin = KeyboardSkin.custom(design, hostDark);
            libraryIds.add(item.id());
            namedKeys.add(skin.key());
            choices.add(new MSIMEInputService.SkinChoice("custom", item.name(), skin, design));
        }
        // 社区里还没获取的皮肤：目录由 App 缓存（键盘不为浏览目录联网），选中时先存进皮肤库再换上。
        java.util.Map<MSIMEInputService.SkinChoice, CommunitySkinCache.Entry> uninstalled = new java.util.HashMap<>();
        for (CommunitySkinCache.Entry entry : communityItems) {
            if (libraryIds.contains(entry.id())) continue;
            KeyboardSkin skin = KeyboardSkin.custom(entry.design(), hostDark);
            if (!namedKeys.add(skin.key())) continue;
            MSIMEInputService.SkinChoice choice = new MSIMEInputService.SkinChoice("custom", entry.name(), skin, entry.design());
            uninstalled.put(choice, entry);
            choices.add(choice);
        }
        choices.removeIf(choice -> "custom".equals(choice.id()) && choice.design() == null
            && namedKeys.contains(choice.skin().key()));
        String globalTheme = InputViewValuePolicy.textOr(preferences, "global_theme", "system");
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
            ViewPolicy.setSelected(card, selected);
            if (selected) selectedIndex = index;
            if ("system".equals(choice.id())) {
                card.setSplitPreview(Color.parseColor(KeyboardSkin.system(false).background()),
                    Color.parseColor(KeyboardSkin.system(true).background()));
            }
            card.setContentDescription("屏幕键盘皮肤 " + choice.title());
            if (Build.VERSION.SDK_INT >= 30)
                card.setStateDescription(selected ? "已选中" : "未选中");
            bindFeedbackAction(card, () -> {
                pendingSkinKey = key;
                for (KeyboardSkinCard other : cards) {
                    ViewPolicy.setSelected(other, other == card);
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
        parent.addView(grid, KeyboardGeometry.matchWidthWrapParams());
        parent.addView(new View(s), KeyboardGeometry.weightedZeroParams(1));
        KeyboardPagerDots dots = new KeyboardPagerDots(s);
        dots.setTag(PAGER_DOTS_TAG);
        dots.setCount(grid.pageCount());
        dots.setActive(initialPage, false);
        if (grid.pageCount() > 1) ViewPolicy.show(dots);
        else ViewPolicy.setInvisible(dots);
        LinearLayout.LayoutParams dotParams = KeyboardGeometry.linearParamsPx(
            s.pixels(Math.round(KeyboardPagerDots.totalWidthDp(
                BoundsPolicy.bounded(grid.pageCount(), 1, Integer.MAX_VALUE)))), s.pixels(10));
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
        LinearLayout root = KeyboardGeometry.column(s);
        KeyboardGeometry.setSymmetricPaddingDp(root, s, 6, 5);
        ViewPolicy.setBackgroundColor(root, Color.parseColor(s.skin.background()));
        root.setContentDescription("高情商回复键盘");

        s.replyHeader = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(s.replyHeader);
        s.replyModeControl = KeyboardGeometry.row(s);
        KeyboardGeometry.setSymmetricPaddingDp(s.replyModeControl, s, 2, 2);
        s.replyReplyModeButton = replySegment("帮你回", "帮你回模式", ReplyKeyboardModel.Mode.REPLY);
        s.replyPolishModeButton = replySegment("帮润色", "帮润色模式", ReplyKeyboardModel.Mode.POLISH);
        s.replyHeader.addView(s.replyModeControl, KeyboardGeometry.linearParamsPx(
            s.pixels(200), LinearLayout.LayoutParams.MATCH_PARENT));
        s.replyHeader.addView(new View(s), KeyboardGeometry.weightedHeightPxParams(1, 1));
        s.replyTemplateButton = s.shortcutButton(s.replyHeader, "模板",
            KeyboardShortcutIconPolicy.Icon.BOOKMARK, this::showReplyTemplates);
        s.replyTemplateButton.setContentDescription("回复模板");
        s.replyTemplateButton.setLayoutParams(KeyboardGeometry.linearParamsPx(
            s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyHeader, KeyboardGeometry.matchWidthHeightPx(s.pixels(36)));

        // 源文字和「粘贴」在同一张卡片里：点文字和点「粘贴」都是粘贴，与 iOS 相同。
        s.replySourceCard = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(s.replySourceCard);
        KeyboardGeometry.setPaddingDp(s.replySourceCard, s, 10, 0, 6, 0);
        s.replySourceButton = MSIMEInputService.role(s.button(s.replySourceCard, MSIMEInputService.REPLY_SOURCE_PLACEHOLDER,
            this::pasteReplySource), KeyboardKeyRole.PLAIN);
        ViewPolicy.setSingleLineEllipsized(s.replySourceButton);
        ViewPolicy.setStartCenteredTextSizeSp(s.replySourceButton, 15);
        s.replySourceButton.setContentDescription("回复源文字");
        compactReplyControl(s.replySourceButton, 0);
        s.replySourceButton.setLayoutParams(KeyboardGeometry.weightedMatchParentParams(1));
        s.replyPasteButton = MSIMEInputService.role(s.button(s.replySourceCard, "粘贴", this::pasteReplySource),
            KeyboardKeyRole.PLAIN);
        s.replyPasteButton.setContentDescription("粘贴回复源文字");
        ViewPolicy.setTextSizeSp(s.replyPasteButton, 13);
        compactReplyControl(s.replyPasteButton, s.pixels(10));
        LinearLayout.LayoutParams pasteParams = KeyboardGeometry.wrapMatchParentParams();
        pasteParams.setMarginStart(s.pixels(6));
        s.replyPasteButton.setLayoutParams(pasteParams);
        root.addView(s.replySourceCard, KeyboardGeometry.matchWidthHeightPx(s.pixels(38)));

        s.replyBody = KeyboardGeometry.row(s);
        s.replyScroll = new ScrollView(s);
        s.replyScroll.setFillViewport(true);
        s.replyScroll.setVerticalScrollBarEnabled(false);
        s.replyMain = KeyboardGeometry.column(s);
        s.replyMain.setContentDescription("回复风格与候选");
        s.replyScroll.addView(s.replyMain);
        s.replyBody.addView(s.replyScroll, KeyboardGeometry.weightedMatchParentParams(1));
        s.replyActions = KeyboardGeometry.column(s);
        s.replyBody.addView(s.replyActions, KeyboardGeometry.linearParamsPx(
            s.pixels(60), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyBody, KeyboardGeometry.weightedWidthParams(1));

        LinearLayout footer = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(footer);
        s.replyProgress = new android.widget.ProgressBar(s, null,
            android.R.attr.progressBarStyleSmall);
        s.replyProgress.setIndeterminate(true);
        ViewPolicy.hide(s.replyProgress);
        LinearLayout.LayoutParams progressParams = KeyboardGeometry.linearParamsPx(
            s.pixels(12), s.pixels(12));
        progressParams.setMarginEnd(s.pixels(4));
        footer.addView(s.replyProgress, progressParams);
        s.replyStatus = aiText("", 11);
        ViewPolicy.setSingleLineEllipsized(s.replyStatus);
        ViewPolicy.clearFontPadding(s.replyStatus);
        s.replyStatus.setContentDescription("高情商回复键盘状态");
        footer.addView(s.replyStatus, KeyboardGeometry.weightedWrapParams(1));
        // 「选风格」只在已有回复时出现，点它回到风格九宫格。
        s.replyStyleResetButton = MSIMEInputService.role(s.button(footer, "选风格", () -> {
            s.replyModel.chooseStyle();
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.GLYPH);
        s.replyStyleResetButton.setContentDescription("重新选择回复风格");
        ViewPolicy.setTextSizeSp(s.replyStyleResetButton, 12);
        compactReplyControl(s.replyStyleResetButton, s.pixels(6));
        s.replyStyleResetButton.setLayoutParams(KeyboardGeometry.wrapMatchParentParams());
        ViewPolicy.hide(s.replyStyleResetButton);
        root.addView(footer, KeyboardGeometry.matchWidthHeightPx(s.pixels(18)));
        return root;
    }

    Button replySegment(String label, String description, ReplyKeyboardModel.Mode mode) {
        Button segment = replyButton(s.replyModeControl, label, description, () -> {
            s.replyModel.setMode(mode);
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        });
        compactReplyControl(segment, 0);
        segment.setLayoutParams(KeyboardGeometry.weightedMatchParentParams(1));
        return segment;
    }

    /** 去掉 Button 自带的最小尺寸、内边距和按下抬升，让回复面板里的控件按自己给定的尺寸排布。 */
    static void compactReplyControl(Button button, int horizontalPadding) {
        ViewPolicy.clearMinimumSize(button);
        ViewPolicy.setHorizontalPadding(button, horizontalPadding);
        ViewPolicy.clearFontPadding(button);
        ViewPolicy.clearStateListAnimator(button);
    }

    Button replyAction(String label, String description, Runnable action) {
        Button button = replyButton(s.replyActions, label, description, action);
        compactReplyControl(button, 0);
        button.setLayoutParams(KeyboardGeometry.weightedWidthParams(1));
        return button;
    }

    private Button replyButton(LinearLayout parent, String label, String description,
            Runnable action) {
        Button button = MSIMEInputService.role(s.button(parent, label, action), KeyboardKeyRole.PLAIN);
        button.setContentDescription(description);
        ViewPolicy.setTextSizeSp(button, 14);
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
        ViewPolicy.setEnabled(s.replyTemplateButton, !busy);
        s.replySourceButton.setText(s.replyModel.source().isEmpty()
            ? MSIMEInputService.REPLY_SOURCE_PLACEHOLDER : s.replyModel.source());
        if (!hasReplies) {
            for (int start = 0; start < ReplyKeyboardModel.STYLES.size(); start += 3) {
                LinearLayout row = KeyboardGeometry.row(s);
                for (int column = 0; column < 3; column++) {
                    ReplyKeyboardModel.Style style = ReplyKeyboardModel.STYLES.get(start + column);
                    Button choice = MSIMEInputService.role(s.button(row, style.emoji() + " " + style.label(),
                        () -> generateReply(style.label())), KeyboardKeyRole.KEY);
                    choice.setContentDescription("回复风格 " + style.label());
                    ViewPolicy.setEnabled(choice, !busy);
                    ViewPolicy.setActiveAlpha(choice, !busy, .45f);
                    ViewPolicy.clearMinimumSize(choice);
                    KeyboardGeometry.setHorizontalPaddingDp(choice, s, 4);
                    ViewPolicy.setMaxLines(choice, 1);
                    ViewPolicy.setAutoSizeSp(choice, 10, 14, 1);
                    choice.setLayoutParams(KeyboardGeometry.weightedMatchParentParams(1));
                }
                s.replyMain.addView(row, KeyboardGeometry.weightedWidthParams(1));
            }
        } else {
            for (String reply : s.replyModel.replies()) {
                Button candidate = MSIMEInputService.role(s.button(s.replyMain, reply, () -> useReply(reply)),
                    KeyboardKeyRole.KEY);
                ViewPolicy.setStartCenteredTextSizeSp(candidate, 15);
                candidate.setContentDescription("回复候选，点按插入");
                ViewPolicy.clearMinimumSize(candidate);
                KeyboardGeometry.setSymmetricPaddingDp(candidate, s, 10, 10);
                candidate.setLayoutParams(KeyboardGeometry.matchWidthWrapParams());
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
        ViewPolicy.setVisible(s.replyProgress, busy);
        ViewPolicy.setVisible(s.replyStyleResetButton, hasReplies);
        applyReplyGeometry();
        s.imeStyler.applySkinToView(s.replyKeyboard);
        styleReplyKeyboard();
    }

    static void selectReplySegment(Button segment, boolean selected) {
        ViewPolicy.setSelected(segment, selected);
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
                BoundsPolicy.nonNegative(radius - s.pixels(2))) : null);
            ViewPolicy.setTextColor(segment, foreground);
            segment.setTypeface(Typeface.create(base, selected ? Typeface.BOLD : Typeface.NORMAL));
            ViewPolicy.clearElevation(segment);
        }
        s.replySourceCard.setBackground(replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        ViewPolicy.setTextColor(s.replySourceButton, s.replyModel.source().isEmpty()
            ? ImeStyler.fade(s.skin.keyForeground(), .55) : foreground);
        s.replyPasteButton.setBackground(new InsetDrawable(replySurface(accent, radius),
            0, s.pixels(6), 0, s.pixels(6)));
        // setBackground 会把 InsetDrawable 的内边距（左右为 0）套到按钮上，冲掉前面设的左右留白，文字就贴着色块边缘；换完背景再设回来。
        KeyboardGeometry.setHorizontalPaddingDp(s.replyPasteButton, s, 12);
        ViewPolicy.setTextColor(s.replyPasteButton, onAccent);
        ViewPolicy.clearElevation(s.replyPasteButton);
        for (int index = 0; index < s.replyActions.getChildCount(); index++) {
            if (!(s.replyActions.getChildAt(index) instanceof Button action)) continue;
            boolean primary = action == s.replyPrimaryAction;
            action.setBackground(replySurface(primary ? accent : ImeStyler.fade(s.skin.keyBackground(), .7), radius));
            ViewPolicy.setTextColor(action, primary ? onAccent : foreground);
            ViewPolicy.clearElevation(action);
        }
        ViewPolicy.setTextColor(s.replyStatus, ImeStyler.fade(s.skin.keyForeground(), .7));
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
        ViewPolicy.show(s.aiPolishContainer);
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
        KeyboardGeometry.setSymmetricPaddingDp(s.aiPolishPanel, s, 10, 6);
        KeyboardGeometry.setPaddingDp(s.aiPolishActions, s, 10, 0, 10, 8);
        LinearLayout header = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(header);
        TextView title = aiText(s.aiOutputText.isEmpty() ? "AI 润色" : "润色结果", 15);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        header.addView(title, KeyboardGeometry.weightedWrapParams(1));
        Button back = MSIMEInputService.role(s.button(header, "返回键盘", s::closeAiPolish), KeyboardKeyRole.GLYPH);
        ViewPolicy.setTextSizeSp(back, 13);
        compactReplyControl(back, s.pixels(8));
        back.setLayoutParams(KeyboardGeometry.linearParamsPx(
            LinearLayout.LayoutParams.WRAP_CONTENT, s.pixels(32)));
        s.aiPolishPanel.addView(header);
        java.util.List<TextView> secondary = new java.util.ArrayList<>(3);
        TextView error = null;
        if (!s.aiError.isEmpty()) {
            error = aiText(s.aiError, 13);
            error.setContentDescription("AI 润色状态");
            s.aiPolishPanel.addView(error);
        }
        if (s.aiRequestConfiguration != null) {
            TextView destination = aiText("发送到 " + s.aiRequestConfiguration.destination() + " · "
                    + s.aiRequestConfiguration.model(), 12);
            destination.setContentDescription("AI 请求目标和模型");
            s.aiPolishPanel.addView(destination);
            secondary.add(destination);
        }
        TextView label = aiText(s.aiOutputText.isEmpty() ? "待发送的选中文字" : "润色结果", 12);
        KeyboardGeometry.setPaddingDp(label, s, 0, 6, 0, 4);
        s.aiPolishPanel.addView(label);
        secondary.add(label);
        TextView content = aiText(s.aiOutputText.isEmpty() ? s.aiSourceText : s.aiOutputText, 15);
        KeyboardGeometry.setSymmetricPaddingDp(content, s, 12, 10);
        content.setContentDescription(s.aiOutputText.isEmpty() ? "待润色文字" : "AI 润色结果");
        s.aiPolishPanel.addView(content, KeyboardGeometry.matchWidthWrapParams());
        Button primary;
        if (s.aiBusy) {
            TextView progress = aiText("正在请求…", 12);
            KeyboardGeometry.setPaddingDp(progress, s, 0, 6, 0, 0);
            s.aiPolishPanel.addView(progress);
            secondary.add(progress);
            primary = s.button(s.aiPolishActions, "取消请求", () -> {
                s.cancelAiRequest();
                renderAiPolish();
            });
        } else if (s.aiOutputText.isEmpty()) {
            primary = s.button(s.aiPolishActions, "发送选中文字", this::sendAiPolish);
            ViewPolicy.setEnabled(primary, s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
        } else {
            primary = s.button(s.aiPolishActions, "替换选中文字", this::replaceAiSelection);
            ViewPolicy.setEnabled(primary, s.aiTargetMatches() && s.aiRequestConfiguration != null
                && s.aiRequestConfiguration.equals(s.aiPolishConfiguration));
        }
        compactReplyControl(primary, 0);
        ViewPolicy.setTextSizeSp(primary, 15);
        primary.setLayoutParams(KeyboardGeometry.matchWidthHeightPx(s.pixels(44)));
        s.imeStyler.applySkin();
        // 换肤遍历之后补上卡片底色、次要字色和主操作的强调色。
        float radius = s.pixels(10);
        content.setBackground(replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        for (TextView text : secondary) ViewPolicy.setTextColor(text, ImeStyler.fade(s.skin.keyForeground(), .6));
        if (error != null) ViewPolicy.setTextColor(error, Color.parseColor(s.skin.accent()));
        boolean busy = s.aiBusy;
        primary.setBackground(replySurface(busy ? ImeStyler.fade(s.skin.keyBackground(), .7)
            : Color.parseColor(s.skin.accent()), radius));
        ViewPolicy.setTextColor(primary, busy ? Color.parseColor(s.skin.keyForeground()) : Color.parseColor(s.skin.onAccent()));
        ViewPolicy.setActiveAlpha(primary, primary.isEnabled(), .45f);
        ViewPolicy.clearElevation(primary);
    }

    private TextView aiText(CharSequence text, float sizeSp) {
        TextView view = ViewPolicy.textLabel(s, text, sizeSp);
        KeyboardGeometry.setKeyTextSize(view, sizeSp);
        return view;
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
        ViewPolicy.show(s.schemeScroll);
    }

    void renderSchemePicker() {
        if (s.schemePanel == null) return;
        s.schemePanel.removeAllViews();
        KeyboardGeometry.setPaddingDp(s.schemePanel, s, 8, 14, 8, 6);
        // 4×2 分页网格：已启用的方案按共享目录的顺序，英文 26 键排在第三格（方案不够时排最后），末尾是「+ 添加语言」。
        java.util.List<KeyboardScheme> schemes = s.visibleSchemes;
        final int englishIndex = BoundsPolicy.bounded(2, 0, schemes.size());
        int cardCount = schemes.size() + 2;
        PagedTileGrid grid = new PagedTileGrid(s);
        grid.setGrid(4, 2);
        grid.setSpacing(56, 16, 4, 4);
        grid.setContentDescription("输入方案卡片区域");
        java.util.List<KeyboardSchemeCard> schemeCards = new java.util.ArrayList<>(cardCount);
        java.util.List<Boolean> cardSelection = new java.util.ArrayList<>(cardCount);
        int selectedIndex = 0;
        for (int index = 0; index < cardCount; index++) {
            final KeyboardSchemeCard card;
            final boolean selected;
            final String title;
            if (index == cardCount - 1) {
                title = "添加语言";
                selected = false;
                card = new KeyboardSchemeCard(s, "+", "", "添加语言");
                bindFeedbackAction(card, () -> {
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
                bindFeedbackAction(card, s::selectEnglishScheme);
                ViewPolicy.setEnabled(card, !s.schemeSaving);
            } else {
                KeyboardScheme scheme = schemes.get(index > englishIndex ? index - 1 : index);
                title = scheme.title(s.wubiProfile);
                selected = !s.dedicatedEnglish && scheme == s.selectedScheme;
                card = new KeyboardSchemeCard(s, scheme.glyph(), scheme.badge(s.wubiProfile), title);
                bindFeedbackAction(card, () -> s.selectKeyboardScheme(scheme));
                ViewPolicy.setEnabled(card, !s.schemeSaving);
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

    private void bindFeedbackAction(View view, Runnable action) {
        ViewPolicy.bindClick(view, () -> {
            s.imeKeyFeedback.playFeedback(view);
            action.run();
        });
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
        // 补读一次：键盘进程没在运行时复制的内容，监听收不到。
        s.captureClipboard(false);
        renderClipboardHistory();
        ViewPolicy.show(s.clipboardScroll);
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
        KeyboardGeometry.setSymmetricPaddingDp(s.clipboardPanel, s, 8, 8);
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!cloudAllowed) s.clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        boolean cloud = s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD;
        java.util.List<TextView> notes = new java.util.ArrayList<>(1);
        // 顶部一行小号操作：本机 / 云端分段（云端可用时）、刷新或清空；返回由工具栏的「返回键盘」负责。
        LinearLayout header = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(header);
        if (cloudAllowed) {
            addClipboardTab(header, CloudClipboardPanelPolicy.TAB_LOCAL, CloudClipboardPanelPolicy.Tab.LOCAL);
            addClipboardTab(header, CloudClipboardPanelPolicy.TAB_CLOUD, CloudClipboardPanelPolicy.Tab.CLOUD);
        }
        header.addView(new View(s), KeyboardGeometry.weightedZeroParams(1));
        if (cloud) {
            Button refresh = clipboardAction(header, "刷新", this::refreshCloudClipboard);
            ViewPolicy.setEnabled(refresh,
                s.cloudClipboardStatus != CloudClipboardPanelPolicy.Status.LOADING);
            refresh.setContentDescription("刷新云剪贴板");
        } else if (s.clipboardHistoryEnabled) {
            // 复制会自动记录，不再需要「保存当前」。
            clipboardAction(header, "清空", s::confirmClearClipboardHistory);
        }
        s.clipboardPanel.addView(header, KeyboardGeometry.matchWidthHeightPx(s.pixels(32)));
        if (cloud) {
            renderCloudClipboard(notes);
        } else if (!s.clipboardHistoryEnabled) {
            notes.add(clipboardNote("剪贴板历史未开启，可在设置中开启"));
        } else {
            try {
                java.util.List<ClipboardHistory.Item> items = s.clipboardHistory.load();
                if (items.isEmpty()) notes.add(clipboardNote("复制的文字会自动出现在这里，点按即可插入\n只保存在本机"));
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
        for (TextView note : notes) ViewPolicy.setTextColor(note, ImeStyler.fade(s.skin.keyForeground(), .6));
    }

    private Button clipboardAction(LinearLayout header, String label, Runnable action) {
        Button button = MSIMEInputService.role(s.button(header, label, action), KeyboardKeyRole.GLYPH);
        ViewPolicy.setTextSizeSp(button, 13);
        compactReplyControl(button, s.pixels(10));
        button.setLayoutParams(KeyboardGeometry.wrapMatchParentParams());
        return button;
    }

    private TextView clipboardNote(String text) {
        TextView note = centeredNote(text, 13);
        KeyboardGeometry.setSymmetricPaddingDp(note, s, 12, 20);
        s.clipboardPanel.addView(note, KeyboardGeometry.matchWidthWrapParams());
        return note;
    }


    private TextView centeredNote(String text, float sizeSp) {
        TextView note = ViewPolicy.centeredText(s, text, sizeSp);
        KeyboardGeometry.setKeyTextSize(note, sizeSp);
        return note;
    }

    /** 剪贴板卡片：键帽色圆角卡，第一行是文字（最多两行），第二行是『已置顶 · 设备 · 时间』，元信息用次要色的小号字（经 span，换肤遍历刷字色时不受影响）。 */
    private Button clipboardCard(String text, String meta, Runnable action) {
        KeyboardPressButton card = ViewPolicy.newPressButton(s);
        card.setKeyboardRole(KeyboardKeyRole.KEY);
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
        ViewPolicy.setStartCenteredTextSizeSp(card, 15);
        ViewPolicy.setMaxLinesEllipsized(card, 3);
        KeyboardGeometry.setSymmetricPaddingDp(card, s, 12, 8);
        ViewPolicy.clearMinimumHeight(card);
        ViewPolicy.clearStateListAnimator(card);
        bindFeedbackAction(card, action);
        LinearLayout.LayoutParams params = KeyboardGeometry.matchWidthWrapParams();
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
        ViewPolicy.setSelected(button, s.clipboardTab == tab);
        button.setContentDescription("剪贴板分类 " + title);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(button.isSelected() ? "已选中" : "未选中");
        ViewPolicy.setActiveAlpha(button, button.isSelected(), .55f);
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
        ViewPolicy.show(s.phraseScroll);
        Runnable load = () -> {
            CommonPhrasesStore.Result result;
            try {
                result = CommonPhrasesStore.load(s);
            } catch (RuntimeException | LinkageError error) {
                result = null;
            }
            int phraseCapacity = result != null && result.ok()
                ? result.document().phrases().size() : 0;
            java.util.List<String> phrases = new java.util.ArrayList<>(phraseCapacity);
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
        KeyboardGeometry.setPaddingDp(panel, s, 8, 4, 8, 8);
        s.imeStyler.applySkinBackground(panel);
        TextView note = null;
        if (message != null) {
            note = centeredNote(message, 14);
            KeyboardGeometry.setPaddingDp(note, s, 12, 24, 12, 24);
            panel.addView(note, KeyboardGeometry.matchWidthWrapParams());
        }
        java.util.List<View> lines = new java.util.ArrayList<>(phrases.size());
        for (String phrase : phrases) {
            KeyboardPressButton row = phraseButton(KeyboardKeyRole.PLAIN, phrase,
                "常用语 " + (phrase.length() > 20 ? phrase.substring(0, 20) : phrase), () -> {
                    if (s.connection == null) return;
                    s.command(2);
                    s.commitText(phrase);
                    s.closeCommonPhrases();
                    s.render();
                });
            ViewPolicy.setStartCenteredTextSizeSp(row, 15);
            ViewPolicy.setMaxLinesEllipsized(row, 2);
            KeyboardGeometry.setSymmetricPaddingDp(row, s, 12, 8);
            panel.addView(row, KeyboardGeometry.matchWidthWrapParams());
            View hairline = new View(s);
            LinearLayout.LayoutParams line = KeyboardGeometry.matchWidthHeightPx(
                KeyboardGeometry.atLeastOnePixel(s, 1));
            KeyboardGeometry.setHorizontalMargins(line, s.pixels(12));
            panel.addView(hairline, line);
            lines.add(hairline);
        }
        if (message == null || EMPTY_PHRASES.equals(message)) {
            // 列表下面总有一个去处：空的时候是「添加常用语」，有内容时是「管理常用语」，都直接打开应用的常用语页去添加、修改和删除（#5673），而不是让人自己退出键盘去找。键盘里没有可输入的文本框，增删改放在应用里。读取中和读取失败时不放。
            String label = CommonPhrasesPanelPolicy.entryLabel(phrases.size());
            KeyboardPressButton manage = phraseButton(KeyboardKeyRole.RETURN, label, label, () -> {
                s.closeCommonPhrases();
                s.openHostPage("PHRASES");
            });
            KeyboardGeometry.setHorizontalPaddingDp(manage, s, 24);
            LinearLayout.LayoutParams params = KeyboardGeometry.wrapParams();
            params.gravity = Gravity.CENTER_HORIZONTAL;
            if (!phrases.isEmpty()) params.topMargin = s.pixels(8);
            panel.addView(manage, params);
        }
        s.imeStyler.applySkin();
        for (View hairline : lines) ViewPolicy.setBackgroundColor(hairline, Color.parseColor(s.skin.hairline()));
        if (note != null) ViewPolicy.setTextColor(note, ImeStyler.fade(s.skin.keyForeground(), .6));
    }

    private KeyboardPressButton phraseButton(KeyboardKeyRole role, String text,
            String description, Runnable action) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        button.setKeyboardRole(role);
        button.setText(text);
        ViewPolicy.setTextSizeSp(button, 15);
        ViewPolicy.setMinimumHeight(button, s.pixels(44));
        ViewPolicy.clearStateListAnimator(button);
        button.setContentDescription(description);
        bindFeedbackAction(button, action);
        return button;
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
        // 文本编辑面板叠在功能面板上面（后加入外框），不先关掉它，功能面板会被盖住。
        s.imeTextEditPanel.close();
        s.localInputToolsOpen = false;
        s.imeFunctionPanel.renderMoreTools();
        ViewPolicy.show(s.moreToolsScroll);
        s.moreToolsScroll.requestFocus();
    }

    void buildSymbolPanel() {
        s.symbolPanel = new SymbolPanelView(s,
            (title, description, action, actionStyle) -> {
                Button button = ViewPolicy.newPressButton(s);
                button.setText(title);
                button.setContentDescription(actionStyle ? description : "按键 " + description);
                s.imeStyler.styleButton(button, actionStyle);
                bindFeedbackAction(button, action);
                return button;
            },
            new SymbolPanelView.Listener() {
                @Override public void insert(String text, boolean wholePair, boolean remember) {
                    if (s.connection == null) return;
                    // 轻点的正是前面自动补上、还在光标右边的那个后半个时跨过它，不再写一个；长按照字面上屏。
                    if (wholePair && s.stepOverPairedSymbol(text)) {
                        if (remember) recordSymbolRecent(text);
                        return;
                    }
                    if (!s.commitText(text, TypingSource.LOCAL)) return;
                    // 符号面板不经过 Engine，成对补全由宿主按同一个共享开关决定，后半个放在光标右边。
                    String closing = wholePair && s.pairedPunctuation
                        ? PairedPunctuationPolicy.symbolClosing(text) : null;
                    if (closing != null) s.commitClosingMark(closing, TypingSource.LOCAL);
                    if (remember) recordSymbolRecent(text);
                }

                @Override public void loadCatalog(SymbolPanelModel.Category category, int offset,
                        SymbolPanelView.CatalogPages pages) {
                    loadSymbolCatalogPage(category, offset, pages);
                }

                @Override public void delete() {
                    if (s.connection != null && !s.command(0))
                        s.deleteCodePointBeforeCursor();
                }

                @Override public void close() { s.closeSymbolPanel(); }

                @Override public void restyle(View view) { s.imeStyler.applySkinToView(view); }
            });
        symbolPreferences = s.getSharedPreferences(SYMBOL_RECENTS_PREFERENCES, Context.MODE_PRIVATE);
        ViewPolicy.hide(s.symbolPanel);
        s.keyboardSurface.addView(s.symbolPanel, KeyboardGeometry.frameMatchParentParams());
    }

    /** 「常用」的使用记录：只存在本机，格式和表情的最近使用一样是一个 JSON 字符串数组。 */
    private List<String> loadSymbolRecents() {
        if (symbolPreferences == null) return List.of();
        String document = symbolPreferences.getString(SYMBOL_RECENTS_KEY, "[]");
        if (document == null || document.length() > 16_384) return List.of();
        try {
            JSONArray values = new JSONArray(document);
            int count = Math.min(values.length(), SymbolPanelModel.RECENTS_LIMIT * 2);
            ArrayList<String> stored = new ArrayList<>(count);
            for (int index = 0; index < count; index++) {
                Object value = values.opt(index);
                if (value instanceof String) stored.add((String) value);
            }
            return SymbolPanelModel.normalizeRecents(stored);
        } catch (JSONException error) {
            return List.of();
        }
    }

    /** 隐私模式和不许个性化学习的输入框不记：「常用」会把在那里输入过什么带到别的输入框里。 */
    private void recordSymbolRecent(String symbol) {
        if (symbolPreferences == null || s.learningSuppressed() || !SymbolPanelModel.recordable(symbol)) return;
        List<String> recents = SymbolPanelModel.recordRecent(loadSymbolRecents(), symbol);
        symbolPreferences.edit().putString(SYMBOL_RECENTS_KEY, new JSONArray(recents).toString()).apply();
        if (s.symbolPanel != null) s.symbolPanel.setRecents(recents);
    }

    /** 在表情目录的工作线程上读一页颜文字或符号目录；与表情面板读的是同一个随包 `msime-others.db`，经同一个 `msime_client_emoji_catalog_request`。 */
    private void loadSymbolCatalogPage(SymbolPanelModel.Category category, int offset,
            SymbolPanelView.CatalogPages pages) {
        String resources = s.emojiResources;
        String query;
        try {
            JSONObject request = new JSONObject().put("category", category.catalog())
                .put("group", category.kaomoji() ? "All" : "")
                .put("offset", offset).put("limit", SymbolPanelModel.CATALOG_PAGE_SIZE).put("cursor", true);
            if (!category.parent().isEmpty()) request.put("parent", category.parent());
            query = request.toString();
        } catch (JSONException error) {
            pages.failed();
            return;
        }
        if (resources.isEmpty()) {
            pages.failed();
            return;
        }
        s.emojiWorker.execute(() -> {
            SymbolCatalogPage page = null;
            try {
                page = decodeSymbolCatalogPage(NativeClient.emojiCatalog(query, resources), offset, category.kaomoji());
            } catch (JSONException | RuntimeException | LinkageError ignored) {
                // 读不出目录时面板只说「暂时不可用」，不把资源路径或目录内容写进任何地方。
            }
            SymbolCatalogPage result = page;
            s.main.post(() -> {
                if (result == null) pages.failed();
                else pages.loaded(result.items(), result.nextOffset(), result.complete());
            });
        });
    }

    private record SymbolCatalogPage(List<String> items, int nextOffset, boolean complete) {}

    private static SymbolCatalogPage decodeSymbolCatalogPage(String response, int offset, boolean kaomoji)
            throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!Boolean.TRUE.equals(envelope.opt("ok"))) throw new JSONException("Symbol catalog unavailable");
        JSONObject value = envelope.getJSONObject("value");
        JSONArray entries = value.getJSONArray("items");
        if (entries.length() > SymbolPanelModel.CATALOG_PAGE_SIZE) throw new JSONException("Symbol catalog page too large");
        ArrayList<String> items = new ArrayList<>(entries.length());
        for (int index = 0; index < entries.length(); index++) {
            Object text = entries.getJSONObject(index).opt("text");
            if (!(text instanceof String) || !SymbolPanelModel.validCatalogText((String) text, kaomoji))
                throw new JSONException("Invalid symbol catalog item");
            items.add((String) text);
        }
        long nextOffset = KeyboardGeometry.strictLong(value.opt("next_offset"), -1);
        Object complete = value.opt("complete");
        if (!(complete instanceof Boolean)
                || !SymbolPanelModel.validCatalogCursor(offset, items.size(), nextOffset, (Boolean) complete))
            throw new JSONException("Invalid symbol catalog cursor");
        return new SymbolCatalogPage(items, (int) nextOffset, (Boolean) complete);
    }

    void buildEmojiPanel() {
        s.emojiPreferences = s.getSharedPreferences(MSIMEInputService.EMOJI_RECENTS_PREFERENCES, Context.MODE_PRIVATE);
        s.emojiRecents = s.loadEmojiRecents();
        // 设计：盖在键区上、不盖顶部一行；上面是每行八个的表情网格（可见三行，可滚动），底栏是 ABC | 分类 | ⌫。高度由 PanelSurface 限定为键区高度。
        s.emojiPanel = KeyboardGeometry.column(s);
        KeyboardGeometry.setSymmetricPaddingDp(s.emojiPanel, s, 6, 4);
        ViewPolicy.setBackgroundColor(s.emojiPanel, Color.parseColor(s.skin.background()));
        s.emojiPanel.setContentDescription("表情面板");
        ViewPolicy.setFocusable(s.emojiPanel, true);
        s.emojiGrid = KeyboardGeometry.column(s);
        s.emojiGridScroll = new ScrollView(s);
        s.emojiGridScroll.setFillViewport(false);
        s.emojiGridScroll.setVerticalScrollBarEnabled(false);
        s.emojiGridScroll.setContentDescription("表情网格；每行八个");
        s.emojiGridScroll.addView(s.emojiGrid, KeyboardGeometry.scrollMatchWidthWrapParams());
        s.emojiGridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            if (scrollY > oldY && !view.canScrollVertically(1)) s.loadEmojiPage();
        });
        // 第一次排布出真实高度后按三行重排格子高度。
        s.emojiGridScroll.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            if (bottom - top != oldBottom - oldTop && s.emojiPickerVisible()) view.post(this::renderEmojiGrid);
        });
        s.emojiPanel.addView(s.emojiGridScroll, KeyboardGeometry.weightedWidthParams(1));
        LinearLayout bar = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(bar);
        Button abc = MSIMEInputService.role(s.button(bar, "ABC", s::closeEmojiPicker), KeyboardKeyRole.ACCENT);
        ViewPolicy.setTextSizeSp(abc, 14);
        compactReplyControl(abc, 0);
        abc.setContentDescription("返回键盘");
        abc.setLayoutParams(KeyboardGeometry.linearParams(s, 60, 40));
        s.emojiTabs = KeyboardGeometry.row(s);
        s.emojiTabs.setContentDescription("表情分类");
        LinearLayout.LayoutParams tabsParams = KeyboardGeometry.weightedHeightPxParams(s.pixels(40), 1);
        KeyboardGeometry.setHorizontalMargins(tabsParams, s.pixels(6));
        bar.addView(s.emojiTabs, tabsParams);
        Button deleteEmoji = MSIMEInputService.role(s.button(bar, "⌫", this::deleteFromEmojiPicker),
            KeyboardKeyRole.ACCENT);
        ViewPolicy.setTextSizeSp(deleteEmoji, 18);
        compactReplyControl(deleteEmoji, 0);
        deleteEmoji.setContentDescription("删除");
        deleteEmoji.setLayoutParams(KeyboardGeometry.linearParams(s, 60, 40));
        s.emojiPanel.addView(bar, KeyboardGeometry.matchWidthHeightPx(s.pixels(46)));
        ViewPolicy.hide(s.emojiPanel);
        s.keyboardSurface.addView(s.emojiPanel, KeyboardGeometry.frameMatchParentParams());
    }
}
