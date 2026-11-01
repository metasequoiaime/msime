package app.msime.android;

import app.msime.android.LayoutPolicy;


import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.SharedPreferences;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.InsetDrawable;
import android.os.Build;
import android.view.Gravity;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.ScrollView;
import android.widget.TextView;
import java.util.ArrayList;
import java.util.List;
import java.time.ZoneId;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 盖在键盘上的各个面板：表情、符号、皮肤、输入方案、剪贴板、回复键盘、AI 润色、本地输入菜单与更多工具的入口；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImePanels {
    private static final String SYMBOL_RECENTS_PREFERENCES = "android-symbol-recents";
    private static final String SYMBOL_RECENTS_KEY = "items";
    private final MSIMEInputService s;
    private final ClipboardTextPreview clipboardPreview;
    private SharedPreferences symbolPreferences;
    // 只在主线程上用（打开符号面板、点符号记进「常用」时）；`emojiGlyphPaint` 归 `emojiWorker`，Paint 不能跨线程共用。
    private final Paint symbolRecentsGlyphPaint = new Paint();
    private int replyReadGeneration;
    private int replyMenuReadGeneration;
    /** 符号集插件目录读取的代次；换输入框或关闭面板时作废旧结果。 */
    private long pluginSymbolReadGeneration;

    ImePanels(MSIMEInputService s) {
        this.s = s;
        clipboardPreview = new ClipboardTextPreview(s);
    }

    void dismissClipboardPreview() {
        clipboardPreview.dismiss();
    }

    void invalidatePluginSymbolGroupReads() {
        pluginSymbolReadGeneration = PluginSymbolLifecyclePolicy.advance(pluginSymbolReadGeneration);
    }

    void invalidateReplyTemplateReads() {
        ReplyTemplateLifecyclePolicy.Generations next =
            ReplyTemplateLifecyclePolicy.advanceOnPanelClose(replyReadGeneration, replyMenuReadGeneration);
        replyReadGeneration = next.replyRead();
        replyMenuReadGeneration = next.menuRead();
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
        if (s.imeStickers.selected()) {
            showEmojiStatus(StickerPackModel.status(s.imeStickers.pack()));
            return;
        }
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
        // 顺序和横划切换共用一份：常用始终排第一，水杉娘可用时排第二。
        for (int category : s.imeStickers.categoryOrder()) {
            if (category == s.imeStickers.category()) addStickerTab();
            else if (category == StickerPackModel.RECENTS_CATEGORY)
                addEmojiTab(EmojiCatalogModel.RECENTS, StickerPackModel.RECENTS_CATEGORY);
            else addEmojiTab(EmojiCatalogModel.categories().get(category), category);
        }
    }

    /** 「水杉娘」分类签：画清单第一张的缩略图，大小、选中底色和其他分类签一样。 */
    private void addStickerTab() {
        ImeStickers.StickerButton tab = emojiButton(new ImeStickers.StickerButton(s, s.pixels(2),
            s.pixels(StickerPackModel.TAB_ICON_DP)), KeyboardKeyRole.PLAIN, false,
            () -> s.selectEmojiCategory(s.imeStickers.category()));
        tab.setImage(s.imeStickers.tabIcon());
        ViewPolicy.setSelected(tab, s.imeStickers.selected());
        tab.setContentDescription(StickerPackModel.TAB_DESCRIPTION);
        if (Build.VERSION.SDK_INT >= 30)
            tab.setStateDescription(tab.isSelected() ? "已选中" : "未选中");
        s.emojiTabs.addView(tab, LayoutPolicy.weightedMatchParentParams(1));
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
        s.emojiTabs.addView(tab, LayoutPolicy.weightedMatchParentParams(1));
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
                ViewPolicy.setBackground(tab,
                    new InsetDrawable(face, s.pixels(2), s.pixels(3), s.pixels(2), s.pixels(3)));
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
        if (s.imeStickers.selected()) {
            renderStickerGrid();
            return;
        }
        // 普通 Emoji 每行八格；组合与含组合的最近页用宽格，长短语按实测文字宽度跨格。
        android.graphics.Paint layoutPaint = new android.graphics.Paint();
        boolean combinations = s.emojiItems.stream().anyMatch(item ->
            EmojiCombinationCatalog.GROUP.equals(item.group()) || !layoutPaint.hasGlyph(item.text()));
        int columns = combinations ? 3 : EmojiCatalogModel.COLUMNS;
        s.emojiGridScroll.setContentDescription(combinations
            ? "表情网格；组合表情自动适配宽度" : "表情网格；每行八个");
        int visible = s.emojiGridScroll == null ? 0 : s.emojiGridScroll.getHeight();
        int width = s.emojiGridScroll.getWidth();
        if (width <= 0) width = s.getResources().getDisplayMetrics().widthPixels - s.pixels(12);
        int rowHeight = visible > 0
            ? BoundsPolicy.atLeast(s.pixels(40), visible / 3) : s.pixels(48);
        if (s.emojiSelectedCategory == StickerPackModel.RECENTS_CATEGORY && s.emojiItems.isEmpty()) {
            TextView hint = ViewPolicy.centeredText(s, "点过的表情会出现在这里", 13);
            s.emojiGrid.addView(hint, LayoutPolicy.matchWidthHeightPx(rowHeight));
        }
        LinearLayout row = null;
        int used = 0;
        for (EmojiCatalogModel.Item item : s.emojiItems) {
            Button cell = emojiCell(item);
            float textWidth = cell.getPaint().measureText(item.text());
            int span = combinations ? Math.min(columns,
                Math.max(1, (int) Math.ceil((textWidth + s.pixels(8)) * columns / width))) : 1;
            if (row == null || used + span > columns) {
                row = LayoutPolicy.row(s);
                row.setWeightSum(columns);
                s.emojiGrid.addView(row, LayoutPolicy.matchWidthHeightPx(rowHeight));
                used = 0;
            }
            if (combinations) {
                float available = (float) width * span / columns - s.pixels(8);
                if (textWidth > available)
                    cell.setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX,
                        cell.getTextSize() * available / textWidth);
            }
            row.addView(cell, LayoutPolicy.weightedMatchParentParams(span));
            used += span;
        }
        renderEmojiStatus();
        s.imeStyler.applySkin();
    }

    /** 水杉娘表情包的网格：正方形格子，竖屏手机一行五格，更宽时按每格约 72 dp 加列；缩略图由 {@link ImeStickers} 按格子大小在后台解码，只给看得见的格子。 */
    private void renderStickerGrid() {
        StickerPackModel.Pack pack = s.imeStickers.pack();
        int width = s.emojiGridScroll.getWidth();
        if (width <= 0) width = s.getResources().getDisplayMetrics().widthPixels - s.pixels(12);
        int columns = StickerPackModel.columns(
            Math.round(width / s.getResources().getDisplayMetrics().density));
        int cell = BoundsPolicy.atLeast(1, width / columns);
        s.emojiGridScroll.setContentDescription(StickerPackModel.gridDescription(columns));
        ArrayList<ImeStickers.StickerButton> cells = new ArrayList<>(pack.stickers().size());
        LinearLayout row = null;
        for (int index = 0; index < pack.stickers().size(); index++) {
            if (index % columns == 0) {
                row = LayoutPolicy.row(s);
                row.setWeightSum(columns);
                s.emojiGrid.addView(row, LayoutPolicy.matchWidthHeightPx(cell));
            }
            int position = index;
            ImeStickers.StickerButton button = emojiButton(new ImeStickers.StickerButton(s,
                s.pixels(StickerPackModel.CELL_PADDING_DP), 0), KeyboardKeyRole.PLAIN, true,
                () -> s.imeStickers.send(position));
            button.setContentDescription(StickerPackModel.description(pack, pack.stickers().get(index)));
            row.addView(button, LayoutPolicy.weightedMatchParentParams(1));
            cells.add(button);
        }
        renderEmojiStatus();
        s.imeStyler.applySkin();
        s.imeStickers.bindGrid(cells, columns, cell,
            StickerPackModel.thumbnailEdge(cell, s.pixels(StickerPackModel.CELL_PADDING_DP)));
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
        return emojiButton(ViewPolicy.newPressButton(s), role, counted, action);
    }

    private <T extends KeyboardPressButton> T emojiButton(T button, KeyboardKeyRole role, boolean counted,
            Runnable action) {
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
        // 表情面板只用资源目录和输入连接，会话还在建时也能打开（#10086）；表情经 commitText 直接上屏，不经引擎。
        if (s.emojiPanel == null || !ToolbarReadinessPolicy.emojiEnabled(s.toolbarSession(),
                s.connection != null, !s.emojiResources.isEmpty())) {
            s.notice("表情目录尚未就绪");
            return;
        }
        s.command(2);
        // 有会话时 command 失败会经 fail 停掉会话、清掉资源目录。
        if (s.connection == null || s.emojiResources.isEmpty()) return;
        // 底行和长按进来的表情不经 closeToolbarPanels，推迟的皮肤、输入方式在这里作废（#10086）。
        s.dropDeferredToolbarPanel();
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
        // 有无使用记录都默认打开常用，异步加载贴纸不改变当前分类。
        s.selectEmojiCategory(StickerPackModel.RECENTS_CATEGORY);
    }

    void showSymbolPanel() {
        if (s.session == 0 || s.connection == null || s.symbolPanel == null) {
            s.notice("符号面板尚未就绪");
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
        s.symbolPanel.resetForPresentation(loadSymbolRecents(),
            SymbolCategoryOrderPolicy.ordered(s.localSettings.text(AndroidLocalSettings.SYMBOL_CATEGORY_ORDER)));
        // 面板原先没有底色，网格空着时直接透出底下的字母键；铺上键盘底图。
        s.imeStyler.applySkinBackground(s.symbolPanel);
        ViewPolicy.show(s.symbolPanel);
        s.symbolPanel.requestFocus();
        loadPluginSymbolGroups();
    }

    /**
     * 在表情目录的工作线程上读一次插件目录里的符号集（`msime_client_emoji_catalog_request` 的 `list_plugin_symbol_groups`，插件目录是偏好目录下的 `plugins/`，设置里的插件页装的就是这一份），读完换进符号面板（{@link SymbolPanelView#setPluginCategories}）。每次打开面板都读：几个 `plugin.toml` 的量，装上或卸载的包下一次打开就跟上。读不出来时面板只有内置分类，不写日志里不该有的东西（插件目录里的内容是用户自己的）。
     */
    private void loadPluginSymbolGroups() {
        String directory = s.preferencesDirectory;
        if (directory.isEmpty()) return;
        long generation = pluginSymbolReadGeneration =
            PluginSymbolLifecyclePolicy.advance(pluginSymbolReadGeneration);
        // 接口要求 resources 是绝对路径，列插件组时并不读它；内置目录还没准备好时随便给一个应用自己的绝对目录。
        String resources = s.emojiResources.isEmpty() ? s.getFilesDir().getAbsolutePath() : s.emojiResources;
        String query;
        try {
            query = new JSONObject().put("category", SymbolPanelModel.SYMBOLS_CATALOG).put("limit", 1)
                .put("list_plugin_symbol_groups", true)
                .put("plugins", new java.io.File(directory, "plugins").getAbsolutePath()).toString();
        } catch (JSONException error) {
            return;
        }
        s.emojiWorker.execute(() -> {
            List<SymbolPanelModel.Category> categories;
            try {
                categories = SymbolPanelModel.pluginCategories(decodePluginSymbolGroups(NativeClient.emojiCatalog(query, resources)));
            } catch (JSONException | RuntimeException | LinkageError error) {
                categories = List.of();
            }
            List<SymbolPanelModel.Category> loaded = categories;
            s.main.post(() -> {
                if (s.symbolPanel != null && PluginSymbolLifecyclePolicy.accepts(
                        generation, pluginSymbolReadGeneration,
                        ViewPolicy.isVisible(s.symbolPanel), directory, s.preferencesDirectory))
                    s.symbolPanel.setPluginCategories(loaded);
            });
        });
    }

    private static List<SymbolPanelModel.PluginGroup> decodePluginSymbolGroups(String response) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!Boolean.TRUE.equals(envelope.opt("ok"))) return List.of();
        JSONArray groups = envelope.getJSONObject("value").optJSONArray("plugin_symbol_groups");
        if (groups == null) return List.of();
        List<SymbolPanelModel.PluginGroup> values = new ArrayList<>(groups.length());
        for (int index = 0; index < groups.length(); index++) {
            JSONObject group = groups.optJSONObject(index);
            if (group == null) continue;
            JSONArray items = group.optJSONArray("items");
            List<String> texts = new ArrayList<>(items == null ? 0 : Math.min(items.length(), SymbolPanelModel.MAX_PLUGIN_GROUP_ITEMS));
            for (int item = 0; items != null && item < items.length() && texts.size() < SymbolPanelModel.MAX_PLUGIN_GROUP_ITEMS; item++) {
                if (items.opt(item) instanceof String text) texts.add(text);
            }
            values.add(new SymbolPanelModel.PluginGroup(JsonPolicy.strictStringOrEmpty(group.opt("pack")),
                JsonPolicy.strictStringOrEmpty(group.opt("pack_name")), JsonPolicy.strictStringOrEmpty(group.opt("tab")),
                JsonPolicy.strictStringOrEmpty(group.opt("title")), texts));
        }
        return values;
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
        int generation = replyReadGeneration = ReplyTemplateLifecyclePolicy.advance(replyReadGeneration);
        if (style != null && style.startsWith("community:")) {
            final CommunityReplyLibrary library = s.communityReplyLibrary;
            s.preferencesWorker.execute(() -> {
                final java.util.List<CommunityReplyLibrary.Template> templates;
                try { templates = library == null ? java.util.List.of() : library.read(); }
                catch (java.io.IOException error) {
                    s.main.post(() -> {
                        if (ReplyTemplateLifecyclePolicy.accepts(generation, replyReadGeneration,
                                s.replyOpen, ViewPolicy.isVisible(s.replyKeyboard))) {
                            s.replyModel.showStatus("回复模板无法读取，请重试");
                            renderReplyKeyboard();
                        }
                    });
                    return;
                }
                s.main.post(() -> {
                    if (ReplyTemplateLifecyclePolicy.accepts(generation, replyReadGeneration,
                            s.replyOpen, ViewPolicy.isVisible(s.replyKeyboard)))
                        generateReplyLoaded(style, templates);
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
                    if (ReplyTemplateLifecyclePolicy.accepts(generation, replyMenuReadGeneration,
                            s.replyOpen, ViewPolicy.isVisible(s.replyKeyboard))) {
                        s.replyModel.showStatus("回复模板无法读取，请重试");
                        renderReplyKeyboard();
                    }
                });
                return;
            }
            s.main.post(() -> {
                if (ReplyTemplateLifecyclePolicy.accepts(generation, replyMenuReadGeneration,
                        s.replyOpen, ViewPolicy.isVisible(s.replyKeyboard)))
                    showReplyTemplatesLoaded(templates);
            });
        });
    }

    private void showReplyTemplatesLoaded(java.util.List<CommunityReplyLibrary.Template> templates) {
        if (templates.isEmpty()) {
            s.notice("请先在 App 社区收藏并添加回复模板");
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
        if (anchor == null || s.skinScroll == null) return;
        // 会话还在建：记下这一下，会话和偏好就绪后打开（#10086）。
        if (s.deferUntilSessionReady(ToolbarOrderPolicy.Item.SKIN)) return;
        if (!s.canSaveKeyboardSkin()) return;
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
        LayoutPolicy.setPaddingDp(s.skinPanel, s, 8, 10, 8, 6);
        // 目录里的全局主题按共享目录的顺序（含水杉四季与春夏秋冬），后面接「我的设计」。
        JSONObject customTheme = preferences == null ? null : preferences.optJSONObject("custom_theme");
        java.util.List<MSIMEInputService.SkinChoice> choices =
            new java.util.ArrayList<>(themes.length());
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = InputViewValuePolicy.textOr(entry, "id", "");
            if (id.isEmpty()) continue;
            // 只列当前明暗下的主题：浅色键盘里不出现深色主题，反之亦然。
            if (!KeyboardSkin.fitsMode(JsonPolicy.strictString(entry.opt("appearance")), hostDark)) continue;
            String themeName = InputViewValuePolicy.textOr(entry, "title", id);
            KeyboardSkin choice = "custom".equals(id) ? s.themeSkin(id, customTheme, hostDark)
                : KeyboardSkin.resolved(entry, themeName, hostDark, null);
            choices.add(new MSIMEInputService.SkinChoice(id, choice.title(), choice, null, null));
        }
        if (choices.isEmpty()) {
            KeyboardSkin system = KeyboardSkin.system(hostDark);
            choices.add(new MSIMEInputService.SkinChoice(system.id(), system.title(), system, null, null));
        }
        // 已获取的设计与社区目录缓存里的设计，按皮肤的绘制键去重；「我的皮肤」与其中某一款相同时只留带名字的那一格。
        java.util.Set<String> libraryIds = new java.util.HashSet<>();
        java.util.Set<String> namedKeys = new java.util.HashSet<>();
        for (CustomSkinLibrary.Item item : libraryItems) {
            JSONObject design = item.design();
            KeyboardSkin skin = KeyboardSkin.custom(design, hostDark);
            libraryIds.add(item.id());
            namedKeys.add(skin.key());
            choices.add(new MSIMEInputService.SkinChoice("custom", item.name(), skin, design, item.id()));
        }
        // 社区里还没获取的皮肤：目录由 App 缓存（键盘不为浏览目录联网），选中时先存进皮肤库再换上。
        java.util.Map<MSIMEInputService.SkinChoice, CommunitySkinCache.Entry> uninstalled = new java.util.HashMap<>();
        for (CommunitySkinCache.Entry entry : communityItems) {
            if (libraryIds.contains(entry.id())) continue;
            KeyboardSkin skin = KeyboardSkin.custom(entry.design(), hostDark);
            if (!namedKeys.add(skin.key())) continue;
            MSIMEInputService.SkinChoice choice = new MSIMEInputService.SkinChoice("custom", entry.name(), skin, entry.design(), entry.id());
            uninstalled.put(choice, entry);
            choices.add(choice);
        }
        choices.removeIf(choice -> "custom".equals(choice.id()) && choice.design() == null
            && namedKeys.contains(choice.skin().key()));
        // 所选主题属于另一种明暗时键盘按跟随系统画，选中态也落在跟随系统上。
        app.msime.android.KeyboardSkinModeChoice.Selected selectedChoice =
            app.msime.android.KeyboardSkinModeChoice.selected(preferences, hostDark, libraryItems);
        String globalTheme = KeyboardSkin.effectiveTheme(themes, selectedChoice.theme(), hostDark);
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
                : "custom".equals(globalTheme) && (selectedChoice.libraryId() != null
                    ? selectedChoice.libraryId().equals(choice.libraryId())
                    : s.skin.key().equals(choice.skin().key()));
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
                        boolean saved = false;
                        try {
                            saved = CustomSkinLibrary.add(java.nio.file.Paths.get(directory),
                                install.id(), install.name(), install.design());
                        } catch (java.io.IOException | RuntimeException error) {
                            android.util.Log.w("MSIMESkin", "Community skin was not saved to the library", error);
                        }
                        boolean installed = saved;
                        s.main.post(() -> {
                            if (installed) s.saveKeyboardSkin(choice.id(), choice.design(), choice.libraryId());
                            else s.notice("皮肤保存失败，保留当前皮肤");
                        });
                    });
                } else s.saveKeyboardSkin(choice.id(), choice.design(), choice.libraryId());
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
        if (ViewPolicy.isVisible(s.skinScroll)) renderSkinPicker();
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
        parent.addView(grid, LayoutPolicy.matchWidthWrapParams());
        parent.addView(new View(s), LayoutPolicy.weightedZeroParams(1));
        KeyboardPagerDots dots = new KeyboardPagerDots(s);
        dots.setTag(PAGER_DOTS_TAG);
        dots.setCount(grid.pageCount());
        dots.setActive(initialPage, false);
        if (grid.pageCount() > 1) ViewPolicy.show(dots);
        else ViewPolicy.setInvisible(dots);
        LinearLayout.LayoutParams dotParams = LayoutPolicy.linearParamsPx(
            s.pixels(Math.round(KeyboardPagerDots.totalWidthDp(
                BoundsPolicy.bounded(grid.pageCount(), 1, Integer.MAX_VALUE)))), s.pixels(PAGER_DOTS_HEIGHT_DP));
        dotParams.gravity = Gravity.CENTER_HORIZONTAL;
        dotParams.topMargin = s.pixels(PAGER_DOTS_MARGIN_DP);
        dotParams.bottomMargin = s.pixels(PAGER_DOTS_MARGIN_DP);
        parent.addView(dots, dotParams);
        grid.setOnPageChangeListener((page, count) -> dots.setActive(page, true));
        grid.post(() -> grid.setPage(initialPage, false));
    }

    /**
     * 按面板高度排行的分页网格（{@link PagedTileGrid#setPickerFit}）加下方页点：输入方式面板用。网格拿去页点以外的全部高度，所以永远放得进面板；打开时停在第 {@code revealIndex} 个条目所在的页，行数要量过高度才知道，页码也就等到排版时才算。
     */
    private void addFittedGrid(LinearLayout parent, PagedTileGrid grid, int revealIndex) {
        parent.addView(grid, LayoutPolicy.weightedWidthParams(1));
        KeyboardPagerDots dots = new KeyboardPagerDots(s);
        dots.setTag(PAGER_DOTS_TAG);
        // 页点占满一行、自己居中画，页数随行数变时不必重新量宽度；只有一页时占着位置不画。
        LinearLayout.LayoutParams dotParams = LayoutPolicy.matchWidthHeightPx(s.pixels(PAGER_DOTS_HEIGHT_DP));
        dotParams.topMargin = s.pixels(PAGER_DOTS_MARGIN_DP);
        dotParams.bottomMargin = s.pixels(PAGER_DOTS_MARGIN_DP);
        parent.addView(dots, dotParams);
        PagedTileGrid.OnPageChangeListener sync = (page, count) -> {
            dots.setCount(count);
            dots.setActive(page, true);
            if (count > 1) ViewPolicy.show(dots);
            else ViewPolicy.setInvisible(dots);
        };
        sync.onPageChanged(grid.page(), grid.pageCount());
        grid.setOnPageChangeListener(sync);
        grid.revealOnLayout(revealIndex);
    }

    private static final String PAGER_DOTS_TAG = "msime-pager-dots";
    /** {@link #addPagedGrid} 里页点那一格的高度和上下边距。 */
    private static final int PAGER_DOTS_HEIGHT_DP = 10;
    private static final int PAGER_DOTS_MARGIN_DP = 6;
    /** 输入方式面板的上下内边距。 */
    private static final int SCHEME_PANEL_PADDING_TOP_DP = 14;
    private static final int SCHEME_PANEL_PADDING_BOTTOM_DP = 6;

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
        LinearLayout root = LayoutPolicy.column(s);
        LayoutPolicy.setSymmetricPaddingDp(root, s, 6, 5);
        ViewPolicy.setBackgroundColor(root, Color.parseColor(s.skin.background()));
        root.setContentDescription("高情商回复键盘");

        s.replyHeader = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(s.replyHeader);
        s.replyModeControl = LayoutPolicy.row(s);
        LayoutPolicy.setSymmetricPaddingDp(s.replyModeControl, s, 2, 2);
        s.replyReplyModeButton = replySegment("帮你回", "帮你回模式", ReplyKeyboardModel.Mode.REPLY);
        s.replyPolishModeButton = replySegment("帮润色", "帮润色模式", ReplyKeyboardModel.Mode.POLISH);
        s.replyHeader.addView(s.replyModeControl, LayoutPolicy.linearParamsPx(
            s.pixels(200), LinearLayout.LayoutParams.MATCH_PARENT));
        s.replyHeader.addView(new View(s), LayoutPolicy.weightedHeightPxParams(1, 1));
        s.replyTemplateButton = s.shortcutButton(s.replyHeader, "模板",
            KeyboardShortcutIconPolicy.Icon.BOOKMARK, this::showReplyTemplates);
        s.replyTemplateButton.setContentDescription("回复模板");
        s.replyTemplateButton.setLayoutParams(LayoutPolicy.linearParamsPx(
            s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyHeader, LayoutPolicy.matchWidthHeightPx(s.pixels(36)));

        // 源文字和「粘贴」在同一张卡片里：点文字和点「粘贴」都是粘贴，与 iOS 相同。
        s.replySourceCard = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(s.replySourceCard);
        LayoutPolicy.setPaddingDp(s.replySourceCard, s, 10, 0, 6, 0);
        s.replySourceButton = MSIMEInputService.role(s.button(s.replySourceCard, MSIMEInputService.REPLY_SOURCE_PLACEHOLDER,
            this::pasteReplySource), KeyboardKeyRole.PLAIN);
        ViewPolicy.setSingleLineEllipsized(s.replySourceButton);
        ViewPolicy.setStartCenteredTextSizeSp(s.replySourceButton, 15);
        s.replySourceButton.setContentDescription("回复源文字");
        compactReplyControl(s.replySourceButton, 0);
        s.replySourceButton.setLayoutParams(LayoutPolicy.weightedMatchParentParams(1));
        s.replyPasteButton = MSIMEInputService.role(s.button(s.replySourceCard, "粘贴", this::pasteReplySource),
            KeyboardKeyRole.PLAIN);
        s.replyPasteButton.setContentDescription("粘贴回复源文字");
        ViewPolicy.setTextSizeSp(s.replyPasteButton, 13);
        compactReplyControl(s.replyPasteButton, s.pixels(10));
        LinearLayout.LayoutParams pasteParams = LayoutPolicy.wrapMatchParentParams();
        pasteParams.setMarginStart(s.pixels(6));
        s.replyPasteButton.setLayoutParams(pasteParams);
        root.addView(s.replySourceCard, LayoutPolicy.matchWidthHeightPx(s.pixels(38)));

        s.replyBody = LayoutPolicy.row(s);
        s.replyScroll = new ScrollView(s);
        s.replyScroll.setFillViewport(true);
        s.replyScroll.setVerticalScrollBarEnabled(false);
        s.replyMain = LayoutPolicy.column(s);
        s.replyMain.setContentDescription("回复风格与候选");
        s.replyScroll.addView(s.replyMain);
        s.replyBody.addView(s.replyScroll, LayoutPolicy.weightedMatchParentParams(1));
        s.replyActions = LayoutPolicy.column(s);
        s.replyBody.addView(s.replyActions, LayoutPolicy.linearParamsPx(
            s.pixels(60), LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(s.replyBody, LayoutPolicy.weightedWidthParams(1));

        LinearLayout footer = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(footer);
        s.replyProgress = new android.widget.ProgressBar(s, null,
            android.R.attr.progressBarStyleSmall);
        s.replyProgress.setIndeterminate(true);
        ViewPolicy.hide(s.replyProgress);
        LinearLayout.LayoutParams progressParams = LayoutPolicy.linearParamsPx(
            s.pixels(12), s.pixels(12));
        progressParams.setMarginEnd(s.pixels(4));
        footer.addView(s.replyProgress, progressParams);
        s.replyStatus = aiText("", 11);
        ViewPolicy.setSingleLineEllipsized(s.replyStatus);
        ViewPolicy.clearFontPadding(s.replyStatus);
        s.replyStatus.setContentDescription("高情商回复键盘状态");
        footer.addView(s.replyStatus, LayoutPolicy.weightedWrapParams(1));
        // 「选风格」只在已有回复时出现，点它回到风格九宫格。
        s.replyStyleResetButton = MSIMEInputService.role(s.button(footer, "选风格", () -> {
            s.replyModel.chooseStyle();
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.GLYPH);
        s.replyStyleResetButton.setContentDescription("重新选择回复风格");
        ViewPolicy.setTextSizeSp(s.replyStyleResetButton, 12);
        compactReplyControl(s.replyStyleResetButton, s.pixels(6));
        s.replyStyleResetButton.setLayoutParams(LayoutPolicy.wrapMatchParentParams());
        ViewPolicy.hide(s.replyStyleResetButton);
        root.addView(footer, LayoutPolicy.matchWidthHeightPx(s.pixels(18)));
        return root;
    }

    Button replySegment(String label, String description, ReplyKeyboardModel.Mode mode) {
        Button segment = replyButton(s.replyModeControl, label, description, () -> {
            s.replyModel.setMode(mode);
            s.clearReplyRequestReferences();
            renderReplyKeyboard();
        });
        compactReplyControl(segment, 0);
        segment.setLayoutParams(LayoutPolicy.weightedMatchParentParams(1));
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
        button.setLayoutParams(LayoutPolicy.weightedWidthParams(1));
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
                LinearLayout row = LayoutPolicy.row(s);
                for (int column = 0; column < 3; column++) {
                    ReplyKeyboardModel.Style style = ReplyKeyboardModel.STYLES.get(start + column);
                    Button choice = MSIMEInputService.role(s.button(row, style.emoji() + " " + style.label(),
                        () -> generateReply(style.label())), KeyboardKeyRole.KEY);
                    choice.setContentDescription("回复风格 " + style.label());
                    ViewPolicy.setEnabled(choice, !busy);
                    ViewPolicy.setActiveAlpha(choice, !busy, .45f);
                    ViewPolicy.clearMinimumSize(choice);
                    LayoutPolicy.setHorizontalPaddingDp(choice, s, 4);
                    ViewPolicy.setMaxLines(choice, 1);
                    ViewPolicy.setAutoSizeSp(choice, 10, 14, 1);
                    choice.setLayoutParams(LayoutPolicy.weightedMatchParentParams(1));
                }
                s.replyMain.addView(row, LayoutPolicy.weightedWidthParams(1));
            }
        } else {
            for (String reply : s.replyModel.replies()) {
                Button candidate = MSIMEInputService.role(s.button(s.replyMain, reply, () -> useReply(reply)),
                    KeyboardKeyRole.KEY);
                ViewPolicy.setStartCenteredTextSizeSp(candidate, 15);
                candidate.setContentDescription("回复候选，点按插入");
                ViewPolicy.clearMinimumSize(candidate);
                LayoutPolicy.setSymmetricPaddingDp(candidate, s, 10, 10);
                candidate.setLayoutParams(LayoutPolicy.matchWidthWrapParams());
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
        ViewPolicy.setBackground(s.replyModeControl,
            replySurface(ImeStyler.fade(s.skin.keyForeground(), .08), radius));
        for (Button segment : new Button[] {s.replyReplyModeButton, s.replyPolishModeButton}) {
            boolean selected = segment.isSelected();
            ViewPolicy.setBackground(segment, selected ? replySurface(Color.parseColor(s.skin.keyBackground()),
                BoundsPolicy.nonNegative(radius - s.pixels(2))) : null);
            ViewPolicy.setTextColor(segment, foreground);
            ViewPolicy.setTypeface(segment,
                Typeface.create(base, selected ? Typeface.BOLD : Typeface.NORMAL));
            ViewPolicy.clearElevation(segment);
        }
        ViewPolicy.setBackground(s.replySourceCard,
            replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        ViewPolicy.setTextColor(s.replySourceButton, s.replyModel.source().isEmpty()
            ? ImeStyler.fade(s.skin.keyForeground(), .55) : foreground);
        ViewPolicy.setBackground(s.replyPasteButton, new InsetDrawable(replySurface(accent, radius),
            0, s.pixels(6), 0, s.pixels(6)));
        // setBackground 会把 InsetDrawable 的内边距（左右为 0）套到按钮上，冲掉前面设的左右留白，文字就贴着色块边缘；换完背景再设回来。
        LayoutPolicy.setHorizontalPaddingDp(s.replyPasteButton, s, 12);
        ViewPolicy.setTextColor(s.replyPasteButton, onAccent);
        ViewPolicy.clearElevation(s.replyPasteButton);
        for (int index = 0; index < s.replyActions.getChildCount(); index++) {
            if (!(s.replyActions.getChildAt(index) instanceof Button action)) continue;
            boolean primary = action == s.replyPrimaryAction;
            ViewPolicy.setBackground(action,
                replySurface(primary ? accent : ImeStyler.fade(s.skin.keyBackground(), .7), radius));
            ViewPolicy.setTextColor(action, primary ? onAccent : foreground);
            ViewPolicy.clearElevation(action);
        }
        ViewPolicy.setTextColor(s.replyStatus, ImeStyler.fade(s.skin.keyForeground(), .7));
        ProgressBarPolicy.setIndeterminateTint(s.replyProgress, accent);
    }

    void showAiPolish() {
        if (s.aiPolishConfiguration == null) {
            s.notice("请先在共享设置中启用并配置 AI 辅助");
            return;
        }
        String selected = s.selectedEditorText();
        if (!s.aiPolishReady() || !AiPolishConfiguration.acceptableText(selected)) {
            s.notice("请先完成当前输入，再选择一万字以内的文字");
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
                || !ViewPolicy.isVisible(s.aiPolishContainer)) return;
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
        LayoutPolicy.setSymmetricPaddingDp(s.aiPolishPanel, s, 10, 6);
        LayoutPolicy.setPaddingDp(s.aiPolishActions, s, 10, 0, 10, 8);
        LinearLayout header = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(header);
        TextView title = aiText(s.aiOutputText.isEmpty() ? "AI 润色" : "润色结果", 15);
        ViewPolicy.setTypeface(title, Typeface.DEFAULT_BOLD);
        header.addView(title, LayoutPolicy.weightedWrapParams(1));
        Button back = MSIMEInputService.role(s.button(header, "返回键盘", s::closeAiPolish), KeyboardKeyRole.GLYPH);
        ViewPolicy.setTextSizeSp(back, 13);
        compactReplyControl(back, s.pixels(8));
        back.setLayoutParams(LayoutPolicy.linearParamsPx(
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
        LayoutPolicy.setPaddingDp(label, s, 0, 6, 0, 4);
        s.aiPolishPanel.addView(label);
        secondary.add(label);
        TextView content = aiText(s.aiOutputText.isEmpty() ? s.aiSourceText : s.aiOutputText, 15);
        LayoutPolicy.setSymmetricPaddingDp(content, s, 12, 10);
        content.setContentDescription(s.aiOutputText.isEmpty() ? "待润色文字" : "AI 润色结果");
        s.aiPolishPanel.addView(content, LayoutPolicy.matchWidthWrapParams());
        Button primary;
        if (s.aiBusy) {
            TextView progress = aiText("正在请求…", 12);
            LayoutPolicy.setPaddingDp(progress, s, 0, 6, 0, 0);
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
        primary.setLayoutParams(LayoutPolicy.matchWidthHeightPx(s.pixels(44)));
        s.imeStyler.applySkin();
        // 换肤遍历之后补上卡片底色、次要字色和主操作的强调色。
        float radius = s.pixels(10);
        ViewPolicy.setBackground(content,
            replySurface(Color.parseColor(s.skin.keyBackground()), radius));
        for (TextView text : secondary) ViewPolicy.setTextColor(text, ImeStyler.fade(s.skin.keyForeground(), .6));
        if (error != null) ViewPolicy.setTextColor(error, Color.parseColor(s.skin.accent()));
        boolean busy = s.aiBusy;
        ViewPolicy.setBackground(primary, replySurface(busy ? ImeStyler.fade(s.skin.keyBackground(), .7)
            : Color.parseColor(s.skin.accent()), radius));
        ViewPolicy.setTextColor(primary, busy ? Color.parseColor(s.skin.keyForeground()) : Color.parseColor(s.skin.onAccent()));
        ViewPolicy.setActiveAlpha(primary, primary.isEnabled(), .45f);
        ViewPolicy.clearElevation(primary);
    }

    private TextView aiText(CharSequence text, float sizeSp) {
        TextView view = ViewPolicy.textLabel(s, text, sizeSp);
        KeyboardTypographyPolicy.setKeyTextSize(view, sizeSp);
        return view;
    }

    void showSchemePicker() {
        // 其他设置正在保存时只是这一下不打开：保存一瞬间就完成，不必弹「尚未就绪」。
        if (s.touchGeometrySaving || s.traditionalOutputSaving) return;
        // 会话还在建：记下这一下，会话和偏好就绪后打开（#10086）。
        if (s.deferUntilSessionReady(ToolbarOrderPolicy.Item.SCHEME)) return;
        if (s.session == 0 || s.preferencesSnapshot == null || s.preferencesDirectory.isEmpty()) {
            s.notice("输入方案尚未就绪");
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
        LayoutPolicy.setPaddingDp(s.schemePanel, s, 8, SCHEME_PANEL_PADDING_TOP_DP, 8, SCHEME_PANEL_PADDING_BOTTOM_DP);
        // 4 列分页网格，手机竖屏和平板三行、手机横屏两行（MSIMEInputService.pickerPhoneLandscape），网格按面板量到的高度定行高和行距，放不下原尺寸时卡片整体缩小（PagedTileGrid.pickerGrid）：已启用的方案按共享目录的顺序，英文 26 键排在第三格（方案不够时排最后），末尾是「+ 添加语言」。
        java.util.List<KeyboardScheme> schemes = s.visibleSchemes;
        final int englishIndex = BoundsPolicy.bounded(2, 0, schemes.size());
        int cardCount = schemes.size() + 2;
        PagedTileGrid grid = new PagedTileGrid(s);
        grid.setGrid(4, 3);
        grid.setSpacing(KeyboardSchemeCard.HEIGHT_DP, PagedTileGrid.PICKER_THREE_ROW_GAP_DP, 4, 4);
        grid.setPickerFit(KeyboardSchemeCard.HEIGHT_DP, s.pickerPhoneLandscape());
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
        addFittedGrid(s.schemePanel, grid, selectedIndex);
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
        boolean cloudAllowed = cloudClipboardTabAvailable();
        if (!CloudClipboardPanelPolicy.panelAvailable(s.clipboardHistoryEnabled, cloudAllowed)
                || s.clipboardSurface == null) return;
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
        s.clipboardTab = CloudClipboardPanelPolicy.initialTab(s.clipboardTab, cloudAllowed);
        clipboardClearPending = false;
        clipboardMoreOpen = false;
        clipboardActionText = null;
        clearClipboardDetail();
        clipboardSwipedText = null;
        clipboardSelectionMode = false;
        clipboardSelectedTexts.clear();
        endClipboardSegmentation();
        resetClipboardCategoryPicker();
        // 补读一次：键盘进程没在运行时复制的内容，监听收不到。
        s.captureClipboard(ClipboardCapturePolicy.Trigger.PANEL_OPENED, false);
        renderClipboardHistory();
        ViewPolicy.show(s.clipboardSurface);
        // Fetched on every opening, whichever half is showing: the local half's 发到云剪贴板 needs to know the account is signed in with the cloud clipboard on.
        if (cloudAllowed) refreshCloudClipboard();
    }

    /**
     * 最近复制那一行的快捷键（#10079）：打开剪贴板面板的本机历史，直接进入这条内容的分词点选（`segment` 为真）或「移动到分类」，和长按这条记录再选「分词」「移动分类」是同一条路。
     *
     * <p>分词和分类都只画在本机分段里，所以先切回本机。打开面板时的补读已经同步把系统剪贴板里这一条记进历史；共享存储拒收的内容（过长、含控制字符）不在历史里，移动分类必然失败，这时直接说明原因，留在历史列表上。
     *
     * @return 真正进入了分词点选或「移动到分类」时为真；面板没打开或这一条不在历史里时为假，调用方据此决定这一条算不算用过。
     */
    boolean openRecentClipAction(String text, boolean segment) {
        s.clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        showClipboardHistory();
        if (!clipboardPanelOpen()) return false;
        if (segment) {
            startClipboardSegmentation(text);
            return true;
        }
        if (!localHistoryContains(text)) {
            s.notice("这条内容不在剪贴板历史里，不能移动分类");
            return false;
        }
        clipboardCategoryMovingText = text;
        renderClipboardHistory();
        if (s.clipboardScroll != null) s.clipboardScroll.scrollTo(0, 0);
        return true;
    }

    /** 本机剪贴板历史里有没有这段文字（共享存储以文字标识条目）；读不出历史时按没有处理。 */
    private boolean localHistoryContains(String text) {
        if (s.clipboardHistory == null) return false;
        try {
            for (ClipboardHistory.Item item : s.clipboardHistory.load())
                if (item.text().equals(text)) return true;
        } catch (IllegalStateException error) {
            return false;
        }
        return false;
    }

    boolean clipboardPanelOpen() {
        return ViewPolicy.isVisible(s.clipboardSurface);
    }

    void clearClipboardDetail() {
        clipboardDetailItem = null;
        clipboardDetailOrigin = ClipboardDetailPolicy.Origin.LIST;
        clipboardDetailScrollY = 0;
    }

    boolean cloudClipboardAllowed() {
        // 输入框本身的限制之外还要过隐私闸门：隐私模式开着时同样不碰云剪贴板。
        return CloudClipboardPanelPolicy.cloudAllowed(s.editorInputType, s.allowLearning)
            && s.imePrivacyGate.pushesCloudClipboard();
    }

    /** 访客模式仍显示云端分段，但只画「此功能开发中」，绝不触网。 */
    private boolean cloudClipboardTabAvailable() {
        return cloudClipboardAllowed() || s.clipboardMode == ClipboardModePolicy.Mode.VISITOR;
    }

    void selectClipboardTab(CloudClipboardPanelPolicy.Tab tab) {
        if (tab == CloudClipboardPanelPolicy.Tab.CLOUD && !cloudClipboardTabAvailable()) return;
        s.clipboardTab = tab;
        clipboardClearPending = false;
        clipboardMoreOpen = false;
        clipboardActionText = null;
        clipboardSwipedText = null;
        clipboardSelectionMode = false;
        clipboardSelectedTexts.clear();
        endClipboardSegmentation();
        resetClipboardCategoryPicker();
        renderClipboardHistory();
    }

    /** 「云端」这一刻归谁：`id` 是水杉账号编号或中继的绑定，`generation` 是账号的登录代次（中继为 0），`relay` 说明走哪条路。 */
    private record CloudOwner(boolean relay, String id, long generation) {}

    /**
     * 「云端」这一刻归谁。偏好里配置了自建中继或 WebDAV 网盘时是中继或网盘，绑定由地址和令牌（网盘是地址和账号）算出（{@link CloudClipboardRelay#binding}），不需要登录；否则是水杉账号的登录绑定，读不到时为 null。
     *
     * <p>只看内存里的偏好快照，不读文件，主线程上可以调用。快照最多晚一秒（偏好每秒重读一次），所以后台线程真正发请求之前还会再问一次共享层（{@link CloudClipboardRelay#selected}），两边不一致就按失败处理，绝不改走账号。
     */
    private CloudOwner cloudClipboardOwner() {
        JSONObject preferences = s.preferencesSnapshot == null ? null : s.preferencesSnapshot.optJSONObject("preferences");
        String relay = CloudClipboardRelay.binding(preferences);
        if (!relay.isEmpty()) return new CloudOwner(true, relay, 0L);
        app.msime.android.SyncSwitch.Binding binding = SyncSignals.binding(s);
        return binding == null ? null : new CloudOwner(false, binding.accountId(), binding.generation());
    }

    /**
     * Ask the service (or the user's own relay) for the cloud list, off the main thread.
     *
     * <p>The answer is drawn only if the field and the panel are still the ones it was asked for; otherwise it is dropped. Errors carry no response text, and nothing about the request is logged.
     *
     * <p>配置了中继就只读中继；面板打开后中继设置变了（换了地址或令牌、改回了账号），这次读取按失败处理，下次刷新时按新的设置来。
     */
    void refreshCloudClipboard() {
        if (s.clipboardMode == ClipboardModePolicy.Mode.VISITOR) {
            s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.VISITOR;
            s.cloudClipboardItems = java.util.List.of();
            if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
            return;
        }
        if (!cloudClipboardAllowed()) return;
        CloudOwner owner = cloudClipboardOwner();
        String accountId = owner == null ? "" : owner.id();
        long bindingGeneration = owner == null ? -1L : owner.generation();
        boolean relay = owner != null && owner.relay();
        s.cloudClipboardAccountId = accountId;
        s.cloudClipboardBindingGeneration = bindingGeneration;
        long generation = ++s.cloudClipboardGeneration;
        s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
        s.cloudClipboardItems = java.util.List.of();
        s.cloudClipboardSessionId = "";
        if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
        if (owner == null || accountId.isEmpty()) {
            s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.SIGNED_OUT;
            if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
            return;
        }
        try {
            s.cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status status;
                java.util.List<BackendAccount.ClipboardItem> items = java.util.List.of();
                String sessionId = "";
                try {
                    // 快照和偏好文件对不上（中继设置刚改过）时抛出 relay_changed，按失败处理，绝不在配置了中继时经账号读。
                    CloudClipboardRelay route = CloudClipboardRelay.forPanel(s, relay, accountId);
                    if (route != null) {
                        BackendAccount.ClipboardPage page = BackendAccount.clipboardPage(
                            route.drive(CloudClipboardE2e.Operation.list("")));
                        status = CloudClipboardPanelPolicy.loaded(page.enabled(), page.pending(), page.items().size());
                        if (CloudClipboardPanelPolicy.showsItems(status)) items = page.items();
                        sessionId = accountId;
                    } else {
                        BackendAccount account = new BackendAccount(s);
                        // Throws when the session owner cannot tell right now, which is a retry, not a sign-in.
                        BackendAccount.SessionCredential session = account.currentSession();
                        if (session.token().isEmpty()) {
                            status = CloudClipboardPanelPolicy.Status.SIGNED_OUT;
                        } else {
                            BackendAccount.ClipboardPage page = account.clipboard("", session.sessionId(),
                                CloudClipboardNativeDriver.local(s));
                            status = CloudClipboardPanelPolicy.loaded(page.enabled(), page.pending(), page.items().size());
                            if (CloudClipboardPanelPolicy.showsItems(status)) items = page.items();
                            sessionId = session.sessionId();
                        }
                    }
                } catch (CloudApi.Failure error) {
                    status = CloudClipboardPanelPolicy.failed(error.status, error.code);
                } catch (BackendAccount.RequestException error) {
                    status = CloudClipboardPanelPolicy.failed(error.status, error.code);
                } catch (Exception | LinkageError error) {
                    status = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status answer = status;
                java.util.List<BackendAccount.ClipboardItem> answered = items;
                String answeredSessionId = sessionId;
                s.main.post(() -> {
                    if (!CloudClipboardPanelPolicy.accepts(generation, s.cloudClipboardGeneration)
                            || !clipboardPanelOpen()) return;
                    if (!acceptsCurrentBinding(accountId, bindingGeneration)) {
                        s.closeClipboardHistory();
                        return;
                    }
                    if (!cloudClipboardAllowed()) {
                        s.closeClipboardHistory();
                        return;
                    }
                    s.cloudClipboardStatus = answer;
                    s.cloudClipboardItems = answered;
                    s.cloudClipboardSessionId = answeredSessionId;
                    if (s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            s.cloudClipboardStatus = CloudClipboardPanelPolicy.Status.FAILED;
        }
    }

    /** Send one local entry to the cloud clipboard (the account's, or the user's own relay when one is set), because the user asked for exactly this one. */
    void uploadClipboardText(String text) {
        boolean cloudAllowed = cloudClipboardAllowed();
        CloudOwner owner = cloudClipboardOwner();
        String accountId = owner == null ? "" : owner.id();
        long bindingGeneration = owner == null ? -1L : owner.generation();
        boolean relay = owner != null && owner.relay();
        if (!CloudClipboardPanelPolicy.acceptsBinding(
                s.cloudClipboardAccountId, s.cloudClipboardBindingGeneration,
                accountId, bindingGeneration) || s.cloudClipboardSessionId.isEmpty()) {
            s.closeClipboardHistory();
            return;
        }
        if (!CloudClipboardPanelPolicy.canUpload(cloudAllowed, s.cloudClipboardStatus, text)) {
            s.notice(CloudClipboardPanelPolicy.explains(s.cloudClipboardStatus)
                    ? CloudClipboardPanelPolicy.message(s.cloudClipboardStatus, 0,
                        CloudClipboardRelayPolicy.webdav(accountId))
                    : "这条记录无法发到云剪贴板");
            return;
        }
        long generation = s.cloudClipboardGeneration;
        String sessionId = s.cloudClipboardSessionId;
        try {
            s.cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status failure = null;
                try {
                    // 中继设置在这一页读出之后改过时抛出 relay_changed：不发，绝不在配置了中继时发到账号，也不发到刚换掉的中继。
                    CloudClipboardRelay route = CloudClipboardRelay.forPanel(s, relay, sessionId);
                    if (route != null) {
                        BackendAccount.clipboardItem(route.drive(CloudClipboardE2e.Operation.add(text)));
                    } else {
                        new BackendAccount(s).addClipboard(text, sessionId, CloudClipboardNativeDriver.local(s));
                    }
                } catch (CloudApi.Failure error) {
                    failure = CloudClipboardPanelPolicy.failed(error.status, error.code);
                } catch (BackendAccount.RequestException error) {
                    failure = CloudClipboardPanelPolicy.failed(error.status, error.code);
                } catch (Exception | LinkageError error) {
                    failure = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status result = failure;
                s.main.post(() -> {
                    if (!CloudClipboardPanelPolicy.acceptsUploadResult(
                            generation, s.cloudClipboardGeneration)
                            || !clipboardPanelOpen()) return;
                    if (!acceptsCurrentBinding(accountId, bindingGeneration)) {
                        s.closeClipboardHistory();
                        return;
                    }
                    s.notice(result == null ? "已发到云剪贴板"
                        : CloudClipboardPanelPolicy.explains(result)
                            ? CloudClipboardPanelPolicy.message(result, 0, CloudClipboardRelayPolicy.webdav(accountId))
                            : "未能发到云剪贴板，请稍后重试");
                    // Re-read rather than splice the entry in: the service deduplicates and orders the list.
                    refreshCloudClipboard();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            s.notice("未能发到云剪贴板，请稍后重试");
        }
    }

    void insertCloudClipboardText(String text) {
        // Re-checked at the tap: the list was drawn for this field, but a field never gets cloud text once it has turned sensitive.
        CloudOwner owner = cloudClipboardOwner();
        if (owner == null || !CloudClipboardPanelPolicy.canUseCloudRows(
                cloudClipboardAllowed(), s.cloudClipboardStatus,
                s.cloudClipboardAccountId, s.cloudClipboardBindingGeneration,
                owner.id(), owner.generation())) {
            s.closeClipboardHistory();
            return;
        }
        s.insertClipboardText(text);
    }

    private boolean acceptsCurrentBinding(String accountId, long bindingGeneration) {
        CloudOwner current = cloudClipboardOwner();
        return current != null && CloudClipboardPanelPolicy.acceptsBinding(
            accountId, bindingGeneration, current.id(), current.generation());
    }

    /** 「更多」菜单里的「清空」点过一次、正在等用户确认；换分段、重开面板都会撤销。 */
    private boolean clipboardClearPending;
    /** 顶行「更多」菜单展开着（#10037）；换分段、重开面板、进入多选、打开分类选择器、长按或查看详情，以及点了菜单里的任何一项都会收起。 */
    private boolean clipboardMoreOpen;
    /** 当前画在面板里的「更多」菜单；收起或面板重画后为 null。换肤遍历之后由 `styleClipboardMoreMenu` 补画外框。 */
    private LinearLayout clipboardMoreMenu;
    /** 长按打开了操作行的那一条（共享存储以文字标识条目）；没有时为 null。 */
    private String clipboardActionText;
    /** 详情页只保留本次打开时的记录快照，返回后重新读取本机历史。 */
    private ClipboardHistory.Item clipboardDetailItem;
    private ClipboardDetailPolicy.Origin clipboardDetailOrigin = ClipboardDetailPolicy.Origin.LIST;
    private int clipboardDetailScrollY;
    /** 最近一次重绘时仍在列表里的长按记录（重新读取的那一份）；从全文预览进入的详情返回时用它重开预览。 */
    private ClipboardHistory.Item clipboardListedActionItem;
    /** 本机历史的多选状态；文字是共享存储识别条目的键。 */
    private boolean clipboardSelectionMode;
    private final java.util.Set<String> clipboardSelectedTexts = new java.util.LinkedHashSet<>();

    /** 选择器关闭或重开后，旧移动操作的回调不能覆盖新面板。 */
    private long clipboardCategoryGeneration;

    void resetClipboardCategoryPicker() {
        clipboardCategoryGeneration++;
        clipboardCategoryChoosing = false;
        clipboardCategoryMovingText = null;
    }

    /** null 为全部，空串为未分类，其余为共享目录中的稳定 ID。 */
    private String clipboardCategory;
    private boolean clipboardCategoryChoosing;
    private String clipboardCategoryMovingText;
    private boolean clipboardCategoryBusy;

    Button buildClipboardBackToTop() {
        Button button = phraseButton(KeyboardKeyRole.KEY, "↑", "返回剪贴板顶部", () -> {
            if (s.clipboardScroll != null) s.clipboardScroll.scrollTo(0, 0);
        });
        button.setContentDescription("返回剪贴板顶部");
        return button;
    }

    void updateClipboardBackToTop() {
        if (s.clipboardBackToTop == null || s.clipboardScroll == null) return;
        boolean shown = s.clipboardScroll.getScrollY() > s.pixels(8)
            && clipboardSegments == null && !clipboardCategoryChoosing && clipboardCategoryMovingText == null;
        if (shown) ViewPolicy.show(s.clipboardBackToTop);
        else ViewPolicy.hide(s.clipboardBackToTop);
        layoutClipboardFloating();
    }

    /** 浮动「撤销」的高度（dp），比 44dp 的「返回顶部」大一号，手指好找好按。 */
    static final int CLIPBOARD_UNDO_HEIGHT_DP = 48;
    /** 当前画着的是本机列表的常态（不在云端、确认清空、多选、详情或分词里），浮动「撤销」只在这时出现，和原来顶行里「撤销」出现的条件相同。 */
    private boolean clipboardUndoAllowed;

    /**
     * 删除记录（单条或多选，#10080）后浮在剪贴板面板右下角的大号「撤销」（#10073）。原来「撤销」排在可横滑的顶行末尾，常见宽度下要先往左滑才看得到，5 秒里来不及；现在固定在右下角，按钮上倒数剩余秒数，归零后消失。
     *
     * <p>用回车键的强调键帽，和卡片、顶行的普通按钮区分开，由换肤遍历按当前皮肤上色。
     */
    Button buildClipboardUndo() {
        int remaining = ClipboardUndoPolicy.DEFAULT_SECONDS;
        Button button = phraseButton(KeyboardKeyRole.RETURN, ClipboardUndoPolicy.buttonLabel(remaining),
            ClipboardUndoPolicy.buttonDescription(remaining), () -> {
                // 撤销会就地重画面板，先收起「更多」，和多选、长按一样不留着展开的菜单。
                clipboardMoreOpen = false;
                s.undoClipboardItems();
            });
        ViewPolicy.setTextSizeSp(button, 16);
        LayoutPolicy.setHorizontalPaddingDp(button, s, 20);
        return button;
    }

    /** 按剩余秒数刷新浮动「撤销」：有可撤销的记录、且面板在本机列表常态时显示并更新倒数，否则收起。 */
    void updateClipboardUndo() {
        if (s.clipboardUndoButton == null) return;
        int remaining = s.clipboardUndoRemainingSeconds();
        if (remaining > 0 && clipboardUndoAllowed) {
            s.clipboardUndoButton.setText(ClipboardUndoPolicy.buttonLabel(remaining));
            s.clipboardUndoButton.setContentDescription(ClipboardUndoPolicy.buttonDescription(remaining));
            ViewPolicy.show(s.clipboardUndoButton);
        } else {
            ViewPolicy.hide(s.clipboardUndoButton);
        }
        layoutClipboardFloating();
    }

    /**
     * 「撤销」占住右下角；「返回顶部」和它同时出现时挪到它正上方，两者不重叠，「撤销」收起后再落回角上。
     *
     * <p>「撤销」显示期间列表底部多留出按钮高度加 8dp 的内边距：否则列表刚好不滚动或双列时，右下角那张卡片被按钮盖住、点不到，横滑露出的删除、固定也按不到，最长要等 9 秒。
     */
    private void layoutClipboardFloating() {
        if (s.clipboardBackToTop == null || s.clipboardUndoButton == null) return;
        boolean undoShown = ViewPolicy.isVisible(s.clipboardUndoButton);
        if (s.clipboardPanel != null) {
            LayoutPolicy.setPaddingDp(s.clipboardPanel, s, 8, 8, 8, undoShown ? 8 + CLIPBOARD_UNDO_HEIGHT_DP + 8 : 8);
        }
        int bottom = s.pixels(8);
        if (undoShown) bottom += s.pixels(CLIPBOARD_UNDO_HEIGHT_DP + 8);
        FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) s.clipboardBackToTop.getLayoutParams();
        if (params == null || params.bottomMargin == bottom) return;
        params.bottomMargin = bottom;
        s.clipboardBackToTop.setLayoutParams(params);
    }

    private String clipboardCategoryLabel() {
        if (clipboardCategory == null) return "本机 ▾";
        if (clipboardCategory.isEmpty()) return "未分类 ▾";
        for (ClipboardHistoryStore.Category category : s.clipboardHistory.categories())
            if (category.id().equals(clipboardCategory)) return category.name() + " ▾";
        clipboardCategory = null;
        return "本机 ▾";
    }

    /** 选择器画在原面板内，避免可取焦点的对话框抢走原编辑器。 */
    private void renderClipboardCategoryPicker() {
        boolean moving = clipboardCategoryMovingText != null;
        clipboardNote(moving ? "移动到分类" : "选择本机分类");
        if (!moving) addClipboardCategoryChoice("全部记录", null);
        addClipboardCategoryChoice("未分类", "");
        for (ClipboardHistoryStore.Category category : s.clipboardHistory.categories())
            addClipboardCategoryChoice(category.name(), category.id());
        LinearLayout actions = addClipboardItemActionRow();
        clipboardItemAction(actions, "分类管理", s::openClipboardCategories);
        clipboardItemAction(actions, "取消", () -> {
            resetClipboardCategoryPicker();
            renderClipboardHistory();
        });
    }

    private void addClipboardCategoryChoice(String name, String id) {
        Button button = phraseButton(KeyboardKeyRole.KEY, name, "剪贴板目标分类 " + name, () -> {
            if (clipboardCategoryBusy) return;
            if (clipboardCategoryMovingText != null) {
                moveClipboardCategory(clipboardCategoryMovingText, id == null || id.isEmpty() ? null : id);
            } else {
                clipboardCategory = id;
                clipboardCategoryChoosing = false;
                clipboardActionText = null;
                clipboardSwipedText = null;
                clipboardSelectionMode = false;
                clipboardSelectedTexts.clear();
                renderClipboardHistory();
                s.clipboardScroll.scrollTo(0, 0);
            }
        });
        button.setContentDescription("剪贴板目标分类 " + name);
        ViewPolicy.setEnabled(button, !clipboardCategoryBusy);
        s.clipboardPanel.addView(button, LayoutPolicy.matchWidthWrapParams());
    }

    private void moveClipboardCategory(String text, String id) {
        clipboardCategoryBusy = true;
        long request = ++clipboardCategoryGeneration;
        ClipboardHistoryStore history = s.clipboardHistory;
        renderClipboardHistory();
        try {
            s.preferencesWorker.execute(() -> {
                boolean changed;
                try { changed = history.setCategory(text, id); }
                catch (RuntimeException | LinkageError error) { changed = false; }
                boolean updated = changed;
                s.main.post(() -> {
                    clipboardCategoryBusy = false;
                    if (request != clipboardCategoryGeneration) {
                        if (clipboardPanelOpen()) renderClipboardHistory();
                        return;
                    }
                    clipboardCategoryMovingText = null;
                    if (clipboardPanelOpen()) {
                        s.notice(updated ? "已移动分类" : "移动失败，记录或分类可能已改变");
                        renderClipboardHistory();
                    }
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            clipboardCategoryBusy = false;
            s.notice("暂时无法移动分类");
            renderClipboardHistory();
        }
    }

    void renderClipboardHistory() {
        dismissClipboardPreview();
        clipboardListedActionItem = null;
        if (s.clipboardPanel == null || s.clipboardHistory == null) return;
        s.clipboardHeader.removeAllViews();
        s.clipboardPanel.removeAllViews();
        clipboardCardRow = null;
        clipboardSwipedCell = null;
        clipboardMoreMenu = null;
        LayoutPolicy.setHorizontalPaddingDp(s.clipboardHeader, s, 8);
        LayoutPolicy.setSymmetricPaddingDp(s.clipboardPanel, s, 8, 8);
        // 详情、分词、云端、确认清空和多选都不显示浮动「撤销」；画到本机列表常态时再打开。
        clipboardUndoAllowed = false;
        updateClipboardUndo();
        if (clipboardDetailItem != null) {
            renderClipboardDetail(clipboardDetailItem);
            return;
        }
        boolean cloudAllowed = cloudClipboardTabAvailable();
        if (!cloudAllowed) s.clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        boolean cloud = s.clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD;
        if (!cloud && clipboardSegments != null) {
            renderClipboardSegmentation();
            return;
        }
        java.util.List<TextView> notes = new java.util.ArrayList<>(2);
        // 本机历史在画顶行之前读一次：顶行的条数和下面的卡片用的是同一份，不读第二次。读不出来时 items 为 null，提示照旧画在下面。
        java.util.List<ClipboardHistory.Item> items = null;
        if (!cloud && s.clipboardHistoryEnabled) {
            try {
                items = s.clipboardHistory.load();
                clipboardCategoryLabel();
                if (clipboardCategory != null) {
                    java.util.List<ClipboardHistory.Item> filtered = new java.util.ArrayList<>();
                    for (ClipboardHistory.Item item : items) {
                        if (clipboardCategory.isEmpty() ? item.categoryId() == null
                            : clipboardCategory.equals(item.categoryId())) filtered.add(item);
                    }
                    items = filtered;
                }
            } catch (IllegalStateException error) {
                items = null;
            }
        }
        // 顶部一行小号操作：本机 / 云端分段（云端可用时）、云端的刷新或本机的搜索、多选和「更多」；返回由工具栏的「返回键盘」负责。
        LinearLayout header = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(header);
        boolean confirmingClear = !cloud && s.clipboardHistoryEnabled && clipboardClearPending;
        // 钉在横向滚动外、顶行最右边的那一段（目前只有「更多」）；没有时为 null。
        LinearLayout moreSlot = null;
        boolean selecting = !cloud && s.clipboardHistoryEnabled && clipboardSelectionMode;
        // 等待确认清空时顶行整行给问句和两个按钮，分段先让出位置，窄屏上也放得下。
        if (!confirmingClear && !selecting) {
            Button local = clipboardAction(header, clipboardCategoryLabel(), () -> {
                if (cloud) selectClipboardTab(CloudClipboardPanelPolicy.Tab.LOCAL);
                boolean choosing = !clipboardCategoryChoosing;
                resetClipboardCategoryPicker();
                clipboardCategoryChoosing = choosing;
                clipboardMoreOpen = false;
                clipboardActionText = null;
                clipboardSwipedText = null;
                renderClipboardHistory();
                s.clipboardScroll.scrollTo(0, 0);
            });
            local.setContentDescription("选择本机剪贴板分类");
            ViewPolicy.setSelected(local, !cloud);
            if (cloudAllowed) addClipboardTab(header, CloudClipboardPanelPolicy.TAB_CLOUD, CloudClipboardPanelPolicy.Tab.CLOUD);
        }
        if (!confirmingClear && !selecting) {
            // 分段和右侧按钮之间原来是一块空白，现在居中写「已存条数/上限」（#5905），不另占地方；没有可数的内容时它是空的，布局和原来一样。
            Integer count = cloud
                ? CloudClipboardPanelPolicy.cloudCount(s.cloudClipboardStatus, s.cloudClipboardItems.size())
                : items == null ? null : items.size();
            TextView counter = ViewPolicy.centeredText(s,
                CloudClipboardPanelPolicy.countLabel(s.clipboardTab, count,
                    s.clipboardHistory.limit()), 12);
            KeyboardTypographyPolicy.setKeyTextSize(counter, 12);
            ViewPolicy.setMaxLinesEllipsized(counter, 1);
            String description = CloudClipboardPanelPolicy.countDescription(s.clipboardTab, count,
                s.clipboardHistory.limit());
            if (description.isEmpty()) ViewPolicy.hideFromAccessibility(counter);
            else counter.setContentDescription(description);
            header.addView(counter, LayoutPolicy.weightedMatchParentParams(1));
            notes.add(counter);
        }
        if (cloud) {
            Button refresh = clipboardAction(header, "刷新", this::refreshCloudClipboard);
            ViewPolicy.setEnabled(refresh,
                s.cloudClipboardStatus != CloudClipboardPanelPolicy.Status.LOADING);
            refresh.setContentDescription("刷新云剪贴板");
        } else if (confirmingClear) {
            // 确认画在面板顶行里，不弹对话框：对话框会让输入法进程崩掉或抢走编辑器焦点，键盘随之收起（#5605、#5653）。
            TextView question = ViewPolicy.textLabel(s, "确认清空全部记录（含固定项）？", 13);
            // 问句在 13sp 到 10sp 之间按可用宽度缩小，再放不下才省略；「取消」「清空」宽度固定，永远留在键盘宽度以内（#10030）。
            KeyboardTypographyPolicy.setFittedKeyTextSize(question, 10, 13);
            ViewPolicy.setMaxLinesEllipsized(question, 1);
            LayoutPolicy.setHorizontalPaddingDp(question, s, 6);
            ViewPolicy.setCenteredVertically(question);
            // 问句和两侧按钮一样按 MATCH_PARENT 取高，与顶行同高。原来问句用 `weightedZeroParams` 高度为 0，顶行套在横向滚动里量高拿到的是 AT_MOST，整行跟着塌成 0 高，问句和「取消」「清空」都看不见、手指点不到，清空也就走不到系统认证（#10027）。
            header.addView(question, LayoutPolicy.weightedMatchParentParams(1));
            clipboardAction(header, "取消", () -> {
                clipboardClearPending = false;
                renderClipboardHistory();
            });
            // 确认清空是删除全部记录的动作，用回车键的强调键帽，和普通的文字按钮区分开（#10030）。
            Button confirm = clipboardAction(header, "清空", () -> {
                clipboardClearPending = false;
                // 先把顶行换回普通状态再拉起系统认证：认证没通过或被取消时，面板上不能还留着一行已经失效的确认。
                renderClipboardHistory();
                s.clearClipboardHistory();
            }, KeyboardKeyRole.RETURN);
            confirm.setContentDescription("确认清空剪贴板历史");
        } else if (selecting) {
            TextView selected = ViewPolicy.centeredText(s,
                clipboardSelectedTexts.size() + " 项已选择", 12);
            header.addView(selected, LayoutPolicy.weightedMatchParentParams(1));
            clipboardAction(header, "取消", this::exitClipboardSelection);
            Button export = clipboardAction(header, "导出", () -> {
                java.util.List<String> selectedTexts = new java.util.ArrayList<>(clipboardSelectedTexts);
                exitClipboardSelection();
                s.exportClipboardItems(selectedTexts);
            });
            ViewPolicy.setEnabled(export, !clipboardSelectedTexts.isEmpty());
            // 删除可能先要系统认证（#10408）：只在认证通过、真正删除后退出多选，取消或失败时用户的选择还在。
            Button remove = clipboardAction(header, "删除", () -> s.removeClipboardItems(
                new java.util.ArrayList<>(clipboardSelectedTexts), this::exitClipboardSelection));
            ViewPolicy.setEnabled(remove, !clipboardSelectedTexts.isEmpty());
        } else {
            // 顶行只留常用的搜索和多选，用得少的分类、导出、清空收进最右边的「更多」，常见宽度下一行放得下、不用横滑（#10037）。删除后的「撤销」不在顶行，浮在面板右下角（#10073，`updateClipboardUndo`）。
            clipboardUndoAllowed = s.clipboardHistoryEnabled;
            if (s.clipboardHistoryEnabled) {
                // 「搜索」打开应用里可搜索的剪贴板历史页（#5973）：键盘里没有可输入的文本框，查询框放在应用里。没有可搜的记录（空或读不出）时不放。
                if (items != null && !items.isEmpty()) {
                    // 搜索通过认证后会关掉面板、去应用里的搜索页，重开面板时菜单已经收起，这里不必另外收。
                    Button search = clipboardAction(header, "搜索", s::openClipboardSearch);
                    search.setContentDescription("在水杉应用里搜索剪贴板历史");
                }
                clipboardAction(header, "多选", () -> {
                    resetClipboardCategoryPicker();
                    clipboardMoreOpen = false;
                    clipboardSelectionMode = true;
                    clipboardSelectedTexts.clear();
                    clipboardActionText = null;
                    clipboardSwipedText = null;
                    renderClipboardHistory();
                });
            }
            // 历史没开时菜单里只有「分类」，入口照旧在，和原来顶行一直有「分类」一致。
            // 「更多」不进横向滚动，钉在顶行最右边：云端分段、很长的分类名或大字号让顶行超宽时，横滑的只是左边那些按钮，「更多」始终在键盘宽度以内。
            moreSlot = LayoutPolicy.row(s);
            ViewPolicy.setCenteredVertically(moreSlot);
            Button more = clipboardAction(moreSlot, clipboardMoreOpen ? "更多 ▴" : "更多 ▾", () -> {
                boolean open = !clipboardMoreOpen;
                resetClipboardCategoryPicker();
                clipboardMoreOpen = open;
                clipboardActionText = null;
                clipboardSwipedText = null;
                renderClipboardHistory();
                // 菜单画在列表最上面，滚到中间时要回到顶部才看得见。
                if (open) s.clipboardScroll.scrollTo(0, 0);
            });
            more.setContentDescription("更多剪贴板操作");
            ViewPolicy.setSelected(more, clipboardMoreOpen);
            if (Build.VERSION.SDK_INT >= 30) more.setStateDescription(clipboardMoreOpen ? "已展开" : "已收起");
        }
        if (confirmingClear) {
            // 确认行不放进横向滚动：HorizontalScrollView 按 UNSPECIFIED 宽度量顶行，带权重的问句会被量成整句宽，窄屏或大字号时整行比键盘宽，「清空」被挤到右侧屏外，要横滑才点得到（#10027、#10030）。直接挂在顶栏下按键盘宽度 EXACTLY 量，问句只分到两个按钮剩下的宽度。
            s.clipboardHeader.addView(header, LayoutPolicy.matchWidthHeightPx(s.pixels(32)));
        } else {
            android.widget.HorizontalScrollView headerScroll = new android.widget.HorizontalScrollView(s);
            headerScroll.setHorizontalScrollBarEnabled(false);
            headerScroll.setFillViewport(true);
            headerScroll.addView(header, LayoutPolicy.scrollMatchWidthWrapParams());
            if (moreSlot == null) {
                s.clipboardHeader.addView(headerScroll, LayoutPolicy.matchWidthHeightPx(s.pixels(32)));
            } else {
                // 横向 LinearLayout 先量不带权重的「更多」，滚动区只分到剩下的宽度。
                moreSlot.addView(headerScroll, 0, LayoutPolicy.weightedMatchParentParams(1));
                s.clipboardHeader.addView(moreSlot, LayoutPolicy.matchWidthHeightPx(s.pixels(32)));
            }
        }
        updateClipboardUndo();
        // 分类选择器和「更多」菜单互斥：打开一个就收起另一个，这里再按选择器优先兜一次。
        clipboardMoreMenu = !cloud && !confirmingClear && !selecting && clipboardMoreOpen
            && !clipboardCategoryChoosing && clipboardCategoryMovingText == null
            ? renderClipboardMoreMenu(items) : null;
        if (!cloud && s.clipboardHistoryEnabled && items != null
            && (clipboardCategoryChoosing || clipboardCategoryMovingText != null)) {
            renderClipboardCategoryPicker();
            s.imeStyler.applySkin();
            updateClipboardBackToTop();
            return;
        }
        if (cloud) {
            renderCloudClipboard(notes);
        } else if (!s.clipboardHistoryEnabled) {
            // 原来只有一句「可在设置中开启」，用户在键盘里看到的是一个空面板，以为剪贴板调不出来。开关就放在这里，和设置「隐私」里的「剪贴板历史」是同一项；默认仍是关，记录复制内容要用户自己点开。
            notes.add(clipboardNote("剪贴板历史未开启\n开启后复制的文字会记在这里，只保存在本机"));
            KeyboardPressButton enable = phraseButton(KeyboardKeyRole.RETURN, "开启剪贴板历史",
                "开启剪贴板历史", s::enableClipboardHistory);
            ViewPolicy.setEnabled(enable, !s.panelPreferenceSaving);
            LayoutPolicy.setHorizontalPaddingDp(enable, s, 24);
            LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
            params.gravity = Gravity.CENTER_HORIZONTAL;
            s.clipboardPanel.addView(enable, params);
        } else if (items == null) {
            notes.add(clipboardNote("历史记录或分类无法读取，请稍后重试"));
        } else {
            if (items.isEmpty()) notes.add(clipboardNote(clipboardCategory == null
                ? "复制的文字会自动出现在这里，点按即可插入\n只保存在本机"
                : "此分类还没有记录，长按记录后可移动分类"));
            long now = System.currentTimeMillis();
            int columns = s.clipboardColumns;
            float reveal = ClipboardSwipePolicy.revealWidth(DimensionPolicy.density(s),
                clipboardCellWidth(columns) - (clipboardSelectionMode ? 0 : s.pixels(38)));
            boolean swipedPresent = false;
            ClipboardHistory.Item actionItem = null;
            for (int index = 0; index < items.size(); index++) {
                ClipboardHistory.Item item = items.get(index);
                String categoryName = "本机";
                for (ClipboardHistoryStore.Category category : s.clipboardHistory.categories())
                    if (category.id().equals(item.categoryId())) { categoryName = category.name(); break; }
                String meta = (item.pinned() ? "已置顶 · " : "") + categoryName + " · "
                    + RelativeTimePolicy.timestampAgo(item.timestamp(), now);
                boolean managed = item.text().equals(clipboardActionText);
                boolean swiped = item.text().equals(clipboardSwipedText);
                swipedPresent |= swiped;
                Button card = newClipboardCard(item.text(), meta, managed, () -> {
                    if (clipboardSelectionMode) {
                        if (clipboardSelectedTexts.contains(item.text())) {
                            clipboardSelectedTexts.remove(item.text());
                        } else if (ClipboardSelectionPolicy.canSelect(clipboardSelectedTexts.size())) {
                            clipboardSelectedTexts.add(item.text());
                        } else {
                            s.notice("最多只能选择 " + ClipboardSelectionPolicy.MAX_SELECTIONS + " 项");
                        }
                        renderClipboardHistory();
                        return;
                    }
                    // 有一条左滑打开着时，点任何一张卡片只把它收回，不插入。
                    if (clipboardSwipedText != null) {
                        closeClipboardSwipe();
                        return;
                    }
                    s.insertClipboardText(item.text());
                });
                card.setContentDescription((item.pinned() ? "已置顶；" : "") + "点按插入剪贴板记录，左滑删除，长按预览全文和管理");
                if (clipboardSelectionMode) {
                    card.setContentDescription((clipboardSelectedTexts.contains(item.text()) ? "已选择；" : "")
                        + "点按选择剪贴板记录");
                    ViewPolicy.setSelected(card, clipboardSelectedTexts.contains(item.text()));
                }
                card.setOnLongClickListener(ignored -> {
                    s.imeKeyFeedback.playFeedback(card);
                    clipboardActionText = item.text();
                    clipboardClearPending = false;
                    clipboardMoreOpen = false;
                    clipboardSwipedText = null;
                    renderClipboardHistory();
                    showClipboardItemPreview(item);
                    return true;
                });
                View cell = clipboardSwipeCell(card, item, swiped, reveal);
                if (!clipboardSelectionMode) {
                    LinearLayout withDetail = LayoutPolicy.row(s);
                    Button detail = MSIMEInputService.role(s.button(withDetail, "ⓘ",
                        () -> openClipboardDetail(item, ClipboardDetailPolicy.Origin.LIST)), KeyboardKeyRole.GLYPH);
                    detail.setContentDescription("查看这条剪贴板记录详情");
                    detail.setLayoutParams(new LinearLayout.LayoutParams(s.pixels(38),
                        LinearLayout.LayoutParams.MATCH_PARENT));
                    withDetail.addView(cell, new LinearLayout.LayoutParams(0,
                        LinearLayout.LayoutParams.WRAP_CONTENT, 1));
                    cell = withDetail;
                }
                placeClipboardCell(index, items.size(), cell);
                if (managed) {
                    actionItem = item;
                    clipboardListedActionItem = item;
                }
                // 操作行放在这条所在那一行的下面：双列时跨在两条下方，不插进两条中间。
                boolean rowEnds = index == items.size() - 1
                    || ClipboardLayoutPolicy.row(index + 1, columns) != ClipboardLayoutPolicy.row(index, columns);
                if (rowEnds && actionItem != null) {
                    renderClipboardItemActions(actionItem, cloudAllowed);
                    actionItem = null;
                }
            }
            // 打开着的那一条已经不在了（别处删掉或清空）：不再记着它。
            if (!swipedPresent) clipboardSwipedText = null;
        }
        s.imeStyler.applySkin();
        for (TextView note : notes) ViewPolicy.setTextColor(note, ImeStyler.fade(s.skin.keyForeground(), .6));
        s.clipboardScroll.post(this::updateClipboardBackToTop);
    }

    /**
     * 「更多」菜单：分类、导出、清空竖排成一列，贴着面板右边画在列表最上面，正好落在「更多」下方（#10037）。
     *
     * <p>画在面板里，不弹 `PopupWindow`、`PopupMenu` 或对话框：对话框会让输入法进程崩掉或抢走编辑器焦点（#5605、#5653），面板内展开和分类选择器是同一种做法。每一项是普通键帽（`KeyboardKeyRole.KEY`），字色和底色就是字母键那一对，由换肤遍历按当前皮肤上色；跟随系统的皮肤在系统深色模式下换成深色那一套，所以菜单文字和底色的反差与键盘上的字母一样，不会融成一片。
     */
    private LinearLayout renderClipboardMoreMenu(java.util.List<ClipboardHistory.Item> items) {
        LinearLayout menu = LayoutPolicy.column(s);
        // 容器不设无障碍说明：TalkBack 会在一个不能操作的节点上多停一下，读屏用户直接落到三项按钮上。
        LayoutPolicy.setSymmetricPaddingDp(menu, s, 4, 4);
        ViewPolicy.setMinimumWidth(menu, s.pixels(96));
        clipboardMoreItem(menu, "分类", "管理剪贴板分类", () -> {
            clipboardMoreOpen = false;
            renderClipboardHistory();
            s.openClipboardCategories();
        });
        // 导出和清空的出现条件与原来在顶行时一样：导出要有记录，清空只要历史开着。
        if (s.clipboardHistoryEnabled && items != null && !items.isEmpty()) {
            clipboardMoreItem(menu, "导出", "导出全部剪贴板历史", () -> {
                clipboardMoreOpen = false;
                renderClipboardHistory();
                s.exportClipboardHistory();
            });
        }
        if (s.clipboardHistoryEnabled) {
            // 清空仍走顶行的两段确认：菜单收起，顶行换成问句和「取消」「清空」（#10027、#10030）。
            clipboardMoreItem(menu, "清空", "清空剪贴板历史", () -> {
                resetClipboardCategoryPicker();
                clipboardMoreOpen = false;
                clipboardClearPending = true;
                clipboardActionText = null;
                clipboardSwipedText = null;
                renderClipboardHistory();
            });
        }
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.gravity = Gravity.END;
        params.bottomMargin = s.pixels(8);
        s.clipboardPanel.addView(menu, params);
        return menu;
    }

    /** 列表当前的滚动位置，按「更多」菜单收起之后的布局算。菜单画在列表最上面，从详情返回时它已收起、列表整体上移了菜单那一截；直接记 `getScrollY()` 会让返回后的列表比离开时往下错开约 140 dp，刚看过的那条可能被滚出顶部（#10037）。 */
    private int clipboardScrollYWithoutMoreMenu() {
        int scrollY = s.clipboardScroll.getScrollY();
        LinearLayout menu = clipboardMoreMenu;
        if (menu == null || menu.getParent() != s.clipboardPanel) return scrollY;
        int menuSpace = menu.getHeight();
        if (menu.getLayoutParams() instanceof LinearLayout.LayoutParams params) {
            menuSpace += params.topMargin + params.bottomMargin;
        }
        return Math.max(0, scrollY - menuSpace);
    }

    private void clipboardMoreItem(LinearLayout menu, String label, String description, Runnable action) {
        Button button = clipboardAction(menu, label, action, KeyboardKeyRole.KEY);
        button.setContentDescription(description);
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthHeightDp(s, 36);
        if (menu.getChildCount() > 1) params.topMargin = s.pixels(4);
        button.setLayoutParams(params);
    }

    /** 菜单外框在换肤遍历之后补画：遍历只给按钮和文字上色，不碰容器。由 `ImeStyler.applySkin` 每次换肤后调用，系统深浅色切换时输入法窗口没关、面板不重画，外框也跟着新皮肤换色。底色取面板底色，描边取按键前景色的淡色，深浅色皮肤下都看得出菜单的边界。 */
    void styleClipboardMoreMenu() {
        LinearLayout menu = clipboardMoreMenu;
        // 面板收着时（比如点一条上屏后）菜单还留在树里，不必每次换肤都给它新建一张底图；重开面板会重画并收起菜单。
        if (menu == null || !ViewPolicy.isVisible(s.clipboardSurface)) return;
        ViewPolicy.setBackground(menu, DrawablePolicy.outlined(Color.parseColor(s.skin.background()),
            s.pixels(8), DimensionPolicy.atLeastOnePixel(s, 1), ImeStyler.fade(s.skin.keyForeground(), .25)));
        ViewPolicy.setElevation(menu, s.pixels(2));
    }

    private void exitClipboardSelection() {
        clipboardSelectionMode = false;
        clipboardSelectedTexts.clear();
        renderClipboardHistory();
    }

    /** 本机记录长按后的全文预览：详情、插入、修改都交回这条记录所属的面板。 */
    private void showClipboardItemPreview(ClipboardHistory.Item item) {
        clipboardPreview.show(item.text(), () -> openClipboardDetail(item, ClipboardDetailPolicy.Origin.PREVIEW),
            () -> s.insertClipboardText(item.text()), () -> {
                clipboardActionText = null;
                s.editClipboardItem(item);
            });
    }

    private void openClipboardDetail(ClipboardHistory.Item item, ClipboardDetailPolicy.Origin origin) {
        clipboardDetailItem = item;
        clipboardDetailOrigin = origin;
        clipboardDetailScrollY = clipboardScrollYWithoutMoreMenu();
        clipboardMoreOpen = false;
        // 从全文预览进入时保留长按展开的操作行，返回后列表和预览都回到进入前的样子。
        if (origin == ClipboardDetailPolicy.Origin.LIST) clipboardActionText = null;
        clipboardSwipedText = null;
        renderClipboardHistory();
        s.clipboardScroll.scrollTo(0, 0);
    }

    /** 详情仍在剪贴板面板内，返回重绘上一级列表，从全文预览进入时再重开预览；都不触碰输入框焦点。 */
    private void renderClipboardDetail(ClipboardHistory.Item item) {
        LinearLayout header = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(header);
        Button back = clipboardAction(header, "返回", () -> {
            returnFromClipboardDetail();
        });
        back.setContentDescription(ClipboardDetailPolicy.backDescription(clipboardDetailOrigin));
        TextView title = ViewPolicy.centeredText(s, "记录详情", 14);
        header.addView(title, LayoutPolicy.weightedMatchParentParams(1));
        s.clipboardHeader.addView(header, LayoutPolicy.matchWidthHeightPx(s.pixels(36)));

        TextView text = ViewPolicy.textLabel(s, item.text(), 16);
        LayoutPolicy.setSymmetricPaddingDp(text, s, 14, 12);
        s.clipboardPanel.addView(text, LayoutPolicy.matchWidthWrapParams());
        String time = ClipboardDetailPolicy.recordedAt(item.timestamp(), ZoneId.systemDefault());
        TextView details = clipboardNote("记录时间：" + time + "\n来源 App："
            + ClipboardDetailPolicy.sourceLabel(null) + "\n包名："
            + ClipboardDetailPolicy.sourceLabel(null));
        details.setGravity(Gravity.START);
        LinearLayout actions = LayoutPolicy.row(s);
        Button insert = clipboardAction(actions, "插入", () -> s.insertClipboardText(item.text()),
            KeyboardKeyRole.RETURN);
        insert.setContentDescription("插入这条剪贴板记录");
        // 底部「返回」和顶栏的那个回到同一级，读屏说明也要一致，否则从预览进入时读屏用户听不出它会重开预览。
        clipboardAction(actions, "返回", this::returnFromClipboardDetail)
            .setContentDescription(ClipboardDetailPolicy.backDescription(clipboardDetailOrigin));
        s.clipboardPanel.addView(actions, LayoutPolicy.matchWidthHeightPx(s.pixels(44)));
        s.imeStyler.applySkin();
        ViewPolicy.setTextColor(details, ImeStyler.fade(s.skin.keyForeground(), .8));
    }

    private void returnFromClipboardDetail() {
        int scrollY = clipboardDetailScrollY;
        ClipboardDetailPolicy.Origin origin = clipboardDetailOrigin;
        clearClipboardDetail();
        renderClipboardHistory();
        s.clipboardScroll.post(() -> s.clipboardScroll.scrollTo(0, scrollY));
        if (ClipboardDetailPolicy.reopensPreview(origin, clipboardListedActionItem != null))
            showClipboardItemPreview(clipboardListedActionItem);
    }

    /**
     * 长按一条历史后，紧贴在它下方的一行操作。
     *
     * <p>固定记录额外画在卡片右上角的角标，不把「已置顶」挤进文字元信息；长文本和双列布局仍保持原来的可用宽度。
     *
     * <p>原来是 `PopupMenu`：弹出菜单是可获得焦点的窗口，打开时编辑器的窗口失去焦点，在 Via 这类用系统 WebView 的浏览器里键盘随即被收起（#5653）。画在面板里的按钮不会碰窗口焦点。
     */
    private void renderClipboardItemActions(ClipboardHistory.Item item, boolean cloudAllowed) {
        // 操作分两行：一行放不下「添加到常用语」「发到云剪贴板」两个长按钮和其余几个短按钮，挤在一行时每个按钮都被截成省略号。阅读顺序是 固定、删除、编辑、分词、分享、移动分类、添加到常用语、发到云剪贴板、收起。
        LinearLayout row = addClipboardItemActionRow();
        clipboardItemAction(row, item.pinned() ? "取消固定" : "固定", () -> {
            clipboardActionText = null;
            s.setClipboardItemPinned(item);
        });
        clipboardItemAction(row, "删除", () -> {
            clipboardActionText = null;
            s.removeClipboardItem(item);
        });
        // 编辑在应用里做（#5971）：键盘里没有可输入的文本框，增删改放在应用里，和常用语（#5673）一样。
        clipboardItemAction(row, "编辑", () -> {
            clipboardActionText = null;
            s.editClipboardItem(item);
        });
        clipboardItemAction(row, "分词", () -> startClipboardSegmentation(item.text()));
        Button share = clipboardItemAction(row, ClipboardSharePolicy.ACTION,
            () -> s.shareClipboardText(item.text()));
        share.setContentDescription("分享这条剪贴板记录");
        LinearLayout more = addClipboardItemActionRow();
        clipboardItemAction(more, "移动分类", () -> {
            clipboardActionText = null;
            clipboardCategoryMovingText = item.text();
            renderClipboardHistory();
            s.clipboardScroll.scrollTo(0, 0);
        });
        clipboardItemAction(more, CommonPhrasesPanelPolicy.ADD_FROM_CLIPBOARD, () -> {
            clipboardActionText = null;
            renderClipboardHistory();
            addClipboardTextToPhrases(item.text());
        });
        // 只要有云端分段就提供这个操作，让用户发现得了；只有这次打开面板的拉取确认账号已登录且开着云剪贴板时它才真正执行。
        if (cloudAllowed) {
            Button upload = clipboardItemAction(more, CloudClipboardPanelPolicy.UPLOAD_ACTION, () -> {
                clipboardActionText = null;
                renderClipboardHistory();
                uploadClipboardText(item.text());
            });
            ViewPolicy.setEnabledWithAlpha(upload, CloudClipboardPanelPolicy.canUpload(
                cloudAllowed, s.cloudClipboardStatus, item.text()), .45f);
        }
        clipboardItemAction(more, "收起", () -> {
            clipboardActionText = null;
            renderClipboardHistory();
        });
    }

    /** 操作行里的一行：加在面板里当前位置（紧贴被长按那条的下方），返回给调用方往里放按钮。 */
    private LinearLayout addClipboardItemActionRow() {
        LinearLayout row = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(row);
        row.setContentDescription("剪贴板记录操作");
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthHeightPx(s.pixels(38));
        params.topMargin = s.pixels(4);
        s.clipboardPanel.addView(row, params);
        return row;
    }

    /**
     * 长按操作行里的「添加到常用语」（#5909）：把这一条存成无编码常用语，之后在键盘的「常用语」面板里点一下就能上屏。
     *
     * <p>经 {@link CommonPhrasesStore#add} 写入，校验、去重、条数上限和标记同步都在那里；常用语读写要等文件锁，放在 `commonPhrasesWorker` 上（与面板读取同一条线程，不排在建会话的偏好读取和词库同步后面），结果回到主线程显示独立提示，不重画剪贴板列表。先把回车换成换行，仍存不下的按实际原因说明（{@link CommonPhrasesPanelPolicy#clipboardRefusal}）。
     */
    private void addClipboardTextToPhrases(String entry) {
        long request = s.beginClipboardPhraseNotice();
        String text = CommonPhrasesPanelPolicy.clipboardPhraseText(entry);
        String refusal = CommonPhrasesPanelPolicy.clipboardRefusal(text);
        if (refusal != null) {
            s.showClipboardPhraseNotice(request, refusal);
            return;
        }
        try {
            s.commonPhrasesWorker.execute(() -> {
                CommonPhrasesStore.Result result;
                try {
                    result = CommonPhrasesStore.add(s, text);
                } catch (RuntimeException | LinkageError error) {
                    result = null;
                }
                String notice = CommonPhrasesPanelPolicy.clipboardAddNotice(
                    result != null && result.ok(), result == null ? "" : result.failure());
                s.main.post(() -> s.showClipboardPhraseNotice(request, notice));
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            s.showClipboardPhraseNotice(request, CommonPhrasesStore.failureMessage(""));
        }
    }

    /** 正在分词的那一条切出的片段；没有在分词时为 null。 */
    private java.util.List<ClipboardSegmentation.Segment> clipboardSegments;
    /** 与 {@link #clipboardSegments} 一一对应的选中状态。 */
    private boolean[] clipboardSegmentSelected;
    private boolean clipboardSegmentsTruncated;
    private TextView clipboardSegmentStatus;
    private Button clipboardSegmentSelectAll;
    private Button clipboardSegmentConfirm;
    private Button clipboardSegmentShare;
    private Button clipboardSegmentPhrase;
    private Button clipboardSegmentCopy;

    /** 长按菜单里的「分词」：把这一条切成词片，面板换成点选界面（#5645）。 */
    private void startClipboardSegmentation(String text) {
        clipboardActionText = null;
        clipboardClearPending = false;
        clipboardSwipedText = null;
        clipboardSegments = ClipboardSegmentation.segment(text,
            java.text.BreakIterator.getWordInstance(java.util.Locale.CHINESE));
        clipboardSegmentSelected = new boolean[clipboardSegments.size()];
        clipboardSegmentsTruncated = ClipboardSegmentation.truncated(text);
        renderClipboardHistory();
        if (s.clipboardScroll != null) s.clipboardScroll.scrollTo(0, 0);
    }

    private void endClipboardSegmentation() {
        clipboardSegments = null;
        clipboardSegmentSelected = null;
        clipboardSegmentsTruncated = false;
        clipboardSegmentStatus = null;
        clipboardSegmentSelectAll = null;
        clipboardSegmentConfirm = null;
        clipboardSegmentShare = null;
        clipboardSegmentPhrase = null;
        clipboardSegmentCopy = null;
    }

    /**
     * 分词界面：词片点选，底部状态与操作固定显示。分享、常用语、复制和插入都按原文顺序拼接选中的词片（{@link ClipboardSegmentation#join}）；复制回到键盘并提供最近复制，插入直接上屏。
     *
     * <p>词片放在面板里自己的滚动区，操作行在滚动区外、贴着面板底边：原先操作行是面板的第一行，跟着词片一起滚，长文字滚到末尾去「去尾」时「插入」「取消」已经滚出屏幕（#5645）。外层的剪贴板滚动视图开着 fillViewport，会把面板撑到正好一屏高；滚动区的高度基数给 1 px 而不是 0：外层第一次按不限高度测量时，高度为 0 的加权子视图会按全部词片的高度报上去，面板就比一屏高，操作行又被推到屏幕外。给了固定基数，面板先报一个很小的高度，外层再按一屏高精确测量，剩下的高度全部分给滚动区。面板于是不比一屏高，外层滚不动、也不拦截触摸，滑动交给词片的滚动区。
     */
    private void renderClipboardSegmentation() {
        java.util.List<TextView> notes = new java.util.ArrayList<>(1);
        LinearLayout content = LayoutPolicy.column(s);
        if (clipboardSegmentsTruncated) {
            TextView note = centeredNote("这条记录很长，只对开头 " + ClipboardSegmentation.MAX_CHARS + " 个字分词", 13);
            LayoutPolicy.setSymmetricPaddingDp(note, s, 12, 20);
            content.addView(note, LayoutPolicy.matchWidthWrapParams());
            notes.add(note);
        }
        ClipboardSegmentLayout words = new ClipboardSegmentLayout(s, s.pixels(6),
            clipboardSegmentSelectionListener());
        words.setContentDescription("分词结果，点按或从词片滑动连续选择，空白处滑动滚动");
        for (int index = 0; index < clipboardSegments.size(); index++) {
            ClipboardSegmentation.Segment segment = clipboardSegments.get(index);
            if (segment.separator()) continue;
            words.addView(clipboardSegmentButton(segment.text(), index));
        }
        content.addView(words, LayoutPolicy.matchWidthWrapParams());
        ScrollView wordScroll = new ScrollView(s);
        wordScroll.setContentDescription("分词滚动区域");
        wordScroll.addView(content, LayoutPolicy.scrollMatchWidthWrapParams());
        LinearLayout.LayoutParams scrollParams = LayoutPolicy.linearParamsPx(
            LinearLayout.LayoutParams.MATCH_PARENT, 1);
        scrollParams.weight = 1;
        s.clipboardPanel.addView(wordScroll, scrollParams);
        clipboardSegmentStatus = ViewPolicy.textLabel(s, "", 13);
        KeyboardTypographyPolicy.setKeyTextSize(clipboardSegmentStatus, 13);
        ViewPolicy.setMaxLinesEllipsized(clipboardSegmentStatus, 1);
        LayoutPolicy.setHorizontalPaddingDp(clipboardSegmentStatus, s, 6);
        s.clipboardPanel.addView(clipboardSegmentStatus, LayoutPolicy.matchWidthWrapParams());
        LinearLayout bar = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(bar);
        clipboardSegmentAction(bar, "取消", "取消分词，回到剪贴板历史", () -> {
            endClipboardSegmentation();
            renderClipboardHistory();
        });
        clipboardSegmentShare = clipboardSegmentAction(bar, "分享", "分享选中的词",
            () -> s.shareClipboardText(selectedClipboardSegments()));
        clipboardSegmentPhrase = clipboardSegmentAction(bar, "常用语", "添加选中的词到常用语",
            () -> addClipboardTextToPhrases(selectedClipboardSegments()));
        clipboardSegmentCopy = clipboardSegmentAction(bar, "复制", "复制选中的词并返回键盘",
            () -> s.copyClipboardSelection(selectedClipboardSegments()));
        clipboardSegmentSelectAll = clipboardSegmentAction(bar, "全选", "全选或取消全部词片", () -> {
            boolean select = !allSegmentsSelected();
            for (int index = 0; index < clipboardSegmentSelected.length; index++)
                clipboardSegmentSelected[index] = select;
            renderClipboardHistory();
        });
        clipboardSegmentConfirm = clipboardSegmentAction(bar, "插入", "插入选中的词", () -> {
            String text = selectedClipboardSegments();
            endClipboardSegmentation();
            s.insertClipboardText(text);
        });
        clipboardSegmentConfirm.setContentDescription("插入选中的词");
        LinearLayout.LayoutParams barParams = LayoutPolicy.matchWidthHeightPx(s.pixels(32));
        barParams.topMargin = s.pixels(6);
        s.clipboardPanel.addView(bar, barParams);
        s.imeStyler.applySkin();
        for (TextView note : notes) ViewPolicy.setTextColor(note, ImeStyler.fade(s.skin.keyForeground(), .6));
        refreshClipboardSegmentControls();
    }

    private ClipboardSegmentLayout.SelectionListener clipboardSegmentSelectionListener() {
        return new ClipboardSegmentLayout.SelectionListener() {
            @Override public void changed(View child, boolean selected) {
                if (clipboardSegmentSelected == null) return;
                clipboardSegmentSelected[(Integer) child.getTag()] = selected;
                setSegmentSelected((Button) child, selected);
                s.imeStyler.styleButton((Button) child, KeyboardKeyRole.KEY, s.skin);
            }
            @Override public void refreshed() { refreshClipboardSegmentControls(); }
        };
    }

    private String selectedClipboardSegments() {
        return clipboardSegments == null ? ""
            : ClipboardSegmentation.join(clipboardSegments, clipboardSegmentSelected);
    }

    /** 六个操作等分可用宽度，长文案放在无障碍说明里，避免窄屏挤掉后面的按钮。 */
    private Button clipboardSegmentAction(LinearLayout bar, String label, String description, Runnable action) {
        Button button = clipboardAction(bar, label, action);
        compactReplyControl(button, s.pixels(2));
        button.setLayoutParams(LayoutPolicy.weightedMatchParentParams(1));
        button.setContentDescription(description);
        return button;
    }

    private Button clipboardSegmentButton(String text, int index) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        button.setKeyboardRole(KeyboardKeyRole.KEY);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(text);
        ViewPolicy.setTextSizeSp(button, 15);
        LayoutPolicy.setSymmetricPaddingDp(button, s, 10, 6);
        ViewPolicy.clearMinimumSize(button);
        ViewPolicy.clearStateListAnimator(button);
        // 不能直接用词片文字做描述：换肤遍历按描述前缀「候选 」「按键 」识别候选和按键。
        button.setContentDescription("词片 " + text);
        button.setTag(index);
        setSegmentSelected(button, clipboardSegmentSelected[index]);
        bindFeedbackAction(button, () -> {
            if (clipboardSegmentSelected == null) return;
            clipboardSegmentSelected[index] = !clipboardSegmentSelected[index];
            setSegmentSelected(button, clipboardSegmentSelected[index]);
            // 只重画这一片和底部的操作行，不重排整个面板，长文字滚到中间时位置不跳。
            s.imeStyler.styleButton(button, KeyboardKeyRole.KEY, s.skin);
            refreshClipboardSegmentControls();
        });
        return button;
    }

    private static void setSegmentSelected(Button button, boolean selected) {
        ViewPolicy.setSelected(button, selected);
        if (Build.VERSION.SDK_INT >= 30) button.setStateDescription(selected ? "已选中" : "未选中");
    }

    private boolean allSegmentsSelected() {
        for (int index = 0; index < clipboardSegments.size(); index++)
            if (!clipboardSegments.get(index).separator() && !clipboardSegmentSelected[index]) return false;
        return true;
    }

    private void refreshClipboardSegmentControls() {
        if (clipboardSegments == null || clipboardSegmentStatus == null) return;
        int chosen = 0;
        for (int index = 0; index < clipboardSegments.size(); index++)
            if (!clipboardSegments.get(index).separator() && clipboardSegmentSelected[index]) chosen++;
        clipboardSegmentStatus.setText(chosen == 0 ? "点选要插入的词" : "已选 " + chosen + " 个");
        boolean all = chosen > 0 && allSegmentsSelected();
        clipboardSegmentSelectAll.setText(all ? "全不选" : "全选");
        ViewPolicy.setEnabledWithAlpha(clipboardSegmentConfirm, chosen > 0, .45f);
        ViewPolicy.setEnabledWithAlpha(clipboardSegmentShare, chosen > 0, .45f);
        ViewPolicy.setEnabledWithAlpha(clipboardSegmentPhrase, chosen > 0, .45f);
        ViewPolicy.setEnabledWithAlpha(clipboardSegmentCopy, chosen > 0, .45f);
    }

    /** 操作行里的一个按钮：键帽样式，宽度按字数分，长的「发到云剪贴板」不会被挤成省略号。 */
    private Button clipboardItemAction(LinearLayout row, String label, Runnable action) {
        Button button = MSIMEInputService.role(s.button(row, label, action), KeyboardKeyRole.KEY);
        ViewPolicy.setTextSizeSp(button, 13);
        compactReplyControl(button, s.pixels(4));
        ViewPolicy.setMaxLinesEllipsized(button, 1);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, BoundsPolicy.atLeast(label.length(), 2));
        params.setMarginStart(s.pixels(3));
        params.setMarginEnd(s.pixels(3));
        button.setLayoutParams(params);
        return button;
    }

    private Button clipboardAction(LinearLayout header, String label, Runnable action) {
        return clipboardAction(header, label, action, KeyboardKeyRole.GLYPH);
    }

    private Button clipboardAction(LinearLayout header, String label, Runnable action,
            KeyboardKeyRole role) {
        Button button = MSIMEInputService.role(s.button(header, label, action), role);
        ViewPolicy.setTextSizeSp(button, 13);
        compactReplyControl(button, s.pixels(10));
        button.setLayoutParams(LayoutPolicy.wrapMatchParentParams());
        return button;
    }

    private TextView clipboardNote(String text) {
        TextView note = centeredNote(text, 13);
        LayoutPolicy.setSymmetricPaddingDp(note, s, 12, 20);
        s.clipboardPanel.addView(note, LayoutPolicy.matchWidthWrapParams());
        return note;
    }


    private TextView centeredNote(String text, float sizeSp) {
        TextView note = ViewPolicy.centeredText(s, text, sizeSp);
        KeyboardTypographyPolicy.setKeyTextSize(note, sizeSp);
        return note;
    }

    /** 双列时正在填的那一行；每次渲染从第 0 条起重新建。 */
    private LinearLayout clipboardCardRow;

    /**
     * 剪贴板卡片：键帽色圆角卡，最多显示三行，长文本从长按浮层阅读全文。正文后是『已置顶 · 设备 · 时间』，元信息用次要色的小号字（经 span，换肤遍历刷字色时不受影响）。
     *
     * <p>`index`/`count` 决定它排在哪：单列时一条一行；双列（本地设置 `platform.android.clipboard_columns`，#5642）时两条一行等宽，最后一行只有一条时右边留空，不把它拉满整行。`selected` 的卡片（长按打开了操作行的那条）画成选中样式，元信息不再用次要色，免得压在强调色上看不清。
     */
    private Button clipboardCard(int index, int count, String text, String meta, boolean selected,
            Runnable action) {
        Button card = newClipboardCard(text, meta, selected, action);
        placeClipboardCell(index, count, card);
        return card;
    }

    /** 只造卡片，不放进面板；本机历史要先把它包进左滑容器（{@link #clipboardSwipeCell}）再放。 */
    private Button newClipboardCard(String text, String meta, boolean selected, Runnable action) {
        KeyboardPressButton card = new KeyboardPressButton(s);
        ViewPolicy.setAllCapsFalse(card);
        ViewPolicy.setMaxLinesEllipsized(card, 3);
        card.setKeyboardRole(KeyboardKeyRole.KEY);
        android.text.SpannableStringBuilder label = new android.text.SpannableStringBuilder(text);
        if (!meta.isEmpty()) {
            label.append('\n');
            int start = label.length();
            label.append(meta);
            label.setSpan(new android.text.style.RelativeSizeSpan(.8f), start, label.length(),
                android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            if (!selected) label.setSpan(new android.text.style.ForegroundColorSpan(
                    Color.parseColor(s.skin.secondary())),
                start, label.length(), android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        ViewPolicy.setSelected(card, selected);
        card.setText(label);
        ViewPolicy.setStartCenteredTextSizeSp(card, 15);
        card.setGravity(Gravity.START | Gravity.TOP);
        LayoutPolicy.setSymmetricPaddingDp(card, s, 12, 8);
        ViewPolicy.clearMinimumHeight(card);
        ViewPolicy.clearStateListAnimator(card);
        bindFeedbackAction(card, action);
        return card;
    }

    /** 把第 `index` 条（卡片本身，或包着它的左滑容器）按列数放进面板。 */
    private void placeClipboardCell(int index, int count, View cell) {
        int columns = s.clipboardColumns;
        if (columns <= 1) {
            LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
            params.topMargin = s.pixels(6);
            s.clipboardPanel.addView(cell, params);
            return;
        }
        int column = index % columns;
        if (column == 0 || clipboardCardRow == null) {
            clipboardCardRow = LayoutPolicy.row(s);
            LinearLayout.LayoutParams rowParams = LayoutPolicy.matchWidthWrapParams();
            rowParams.topMargin = s.pixels(6);
            s.clipboardPanel.addView(clipboardCardRow, rowParams);
        }
        clipboardCardRow.addView(cell, clipboardCellParams(column));
        boolean lastInRow = index == count - 1
            || ClipboardLayoutPolicy.row(index + 1, columns) != ClipboardLayoutPolicy.row(index, columns);
        if (lastInRow) {
            // Space 按最小内容高度测量，普通 View 会占满 AT_MOST 高度，把操作行挤出视口。
            for (int empty = column + 1; empty < columns; empty++)
                clipboardCardRow.addView(new android.widget.Space(s), clipboardCellParams(empty));
            clipboardCardRow = null;
        }
    }

    /** 左滑露出删除按钮的那一条（共享存储以文字标识条目）；同一时间只有一条打开，没有时为 null（#5962）。渲染时按它把那张卡片直接画成打开的样子。 */
    private String clipboardSwipedText;
    /** 打开着的那一格，收回时用；每次渲染重建，没有时为 null。 */
    private ClipboardSwipeCell clipboardSwipedCell;

    /**
     * 本机历史的一格：底下靠右是删除按钮，上面盖着卡片。卡片往左滑让开，露出删除按钮，点它才删（#5962）；手势的判定在 {@link ClipboardSwipePolicy}。
     *
     * <p>删除按钮只在卡片让开（拖动中或打开着）时显示，收着时是 `INVISIBLE`：按住卡片时按压动画会把卡片缩小一圈，底下的删除按钮若一直在，就会从卡片边上露出来（在模拟器上看到过）。收着时它也不可点、不被读屏读到。读屏用户和不会滑的用户仍然从长按操作行里的「删除」删。
     */
    // 监听只在横滑时接手，其余按压原样交给卡片，点击由它自己发出，见 ClipboardSwipeCell。
    @android.annotation.SuppressLint("ClickableViewAccessibility")
    private FrameLayout clipboardSwipeCell(Button card, ClipboardHistory.Item item, boolean open, float reveal) {
        FrameLayout cell = new FrameLayout(s);
        KeyboardIconKey trash = new KeyboardIconKey(s, KeyboardIconKey.Kind.TRASH);
        trash.setKeyboardRole(KeyboardKeyRole.ACCENT);
        // 节点 text 留给读屏，图标代替文字画出来。
        trash.setText("删除");
        trash.setContentDescription("删除这条剪贴板记录");
        ViewPolicy.clearMinimumSize(trash);
        ViewPolicy.clearStateListAnimator(trash);
        // 不让删除按钮的文字高度把这一格撑得比卡片高：格高只由卡片决定。
        ViewPolicy.setPadding(trash, 0, 0, 0, 0);
        bindFeedbackAction(trash, () -> {
            clipboardSwipedText = null;
            clipboardSwipedCell = null;
            clipboardActionText = null;
            s.removeClipboardItem(item);
        });
        // 删除按钮比让开的距离窄一点，和卡片之间留出与卡片间距相同的空隙。
        int trashWidth = BoundsPolicy.atLeast(Math.round(reveal) - s.pixels(6), 1);
        FrameLayout.LayoutParams trashParams = new FrameLayout.LayoutParams(trashWidth,
            FrameLayout.LayoutParams.MATCH_PARENT, Gravity.END);
        cell.addView(trash, trashParams);
        cell.addView(card, LayoutPolicy.frameMatchParentParams());
        ClipboardPinMarkView pinMark = new ClipboardPinMarkView(s);
        pinMark.setPinned(item.pinned(), Color.parseColor(s.skin.accent()));
        FrameLayout.LayoutParams pinParams = new FrameLayout.LayoutParams(s.pixels(20), s.pixels(20), Gravity.TOP | Gravity.END);
        pinParams.topMargin = s.pixels(2);
        pinParams.setMarginEnd(s.pixels(2));
        cell.addView(pinMark, pinParams);
        ClipboardSwipeCell swipe = new ClipboardSwipeCell(card, trash, item.text(), reveal);
        card.setTranslationX(open ? -reveal : 0f);
        swipe.showTrash(open);
        if (open) clipboardSwipedCell = swipe;
        card.setOnTouchListener(swipe);
        return cell;
    }

    /** 收回打开着的那一条（有的话）。点任何一张卡片、开始滑另一条时先收回它；长按、换分段、重开面板、清空和分词会整面板重画，靠清掉 {@link #clipboardSwipedText} 收回。 */
    private void closeClipboardSwipe() {
        ClipboardSwipeCell cell = clipboardSwipedCell;
        clipboardSwipedText = null;
        clipboardSwipedCell = null;
        if (cell != null) cell.settle(false);
    }

    /**
     * 一格的横滑。按下、轻微移动、点按和长按都照常交给卡片自己；手指越过 touch slop 且明显是横向（{@link ClipboardSwipePolicy#claims}）时才接手：不让外层面板拦截去滚动，给卡片补一个取消把按下态、待触发的长按和点按撤掉，之后卡片跟手移动，松手按 {@link ClipboardSwipePolicy#settlesOpen} 停在打开或收回。
     *
     * <p>位移动画用自己的 `ObjectAnimator`，不用 `View.animate()`：卡片的按压反馈（`KeyboardPressFeedback`）也走 `animate()`，按下、松开时会 `cancel()` 它，两边共用一个动画器就会互相打断，卡片停在半路、或者一直保持按下的缩小（在模拟器上看到过）。
     *
     * <p>卡片的点击仍由 `KeyboardPressButton` 自己在松手时发出（这里没接手的按压原样交给它），所以监听里不调 `performClick`。
     */
    private final class ClipboardSwipeCell implements View.OnTouchListener {
        private final Button card;
        private final View trash;
        private final String text;
        private final float reveal;
        private final float density;
        private final int touchSlop;
        private android.animation.ObjectAnimator settling;
        private float downX;
        private float downY;
        private float startOffset;
        private boolean openAtDown;
        private boolean dragging;
        private boolean cancelledPress;
        private android.view.VelocityTracker velocity;

        ClipboardSwipeCell(Button card, View trash, String text, float reveal) {
            this.card = card;
            this.trash = trash;
            this.text = text;
            this.reveal = reveal;
            density = DimensionPolicy.density(s);
            touchSlop = android.view.ViewConfiguration.get(s).getScaledTouchSlop();
        }

        @Override public boolean onTouch(View view, android.view.MotionEvent event) {
            switch (event.getActionMasked()) {
                case android.view.MotionEvent.ACTION_DOWN -> {
                    downX = event.getRawX();
                    downY = event.getRawY();
                    // 不在这里停掉正在进行的收回动画：没有拖起来的轻点要让它照常放完，否则卡片会停在半路。
                    openAtDown = card.getTranslationX() < 0;
                    dragging = false;
                    cancelledPress = false;
                    recycleVelocity();
                    velocity = android.view.VelocityTracker.obtain();
                    track(event);
                    return false;
                }
                case android.view.MotionEvent.ACTION_MOVE -> {
                    track(event);
                    float dx = event.getRawX() - downX;
                    float dy = event.getRawY() - downY;
                    if (!dragging) {
                        if (!ClipboardSwipePolicy.claims(dx, dy, touchSlop, openAtDown)) {
                            // 外层列表没有足够内容可滚时不会接手纵向拖动；仍要撤销卡片按压，
                            // 否则手指始终留在卡片边界内，松手会被 Button 当作点按并插入正文。
                            if (!cancelledPress
                                    && Math.max(Math.abs(dx), Math.abs(dy)) > touchSlop) {
                                cancelledPress = true;
                                android.view.MotionEvent cancel =
                                    android.view.MotionEvent.obtain(event);
                                cancel.setAction(android.view.MotionEvent.ACTION_CANCEL);
                                card.onTouchEvent(cancel);
                                cancel.recycle();
                            }
                            return false;
                        }
                        dragging = true;
                        cancelledPress = true;
                        // 接手时卡片可能还在收回的动画里：停在当前位置，再从这里跟手，不跳。
                        stopSettling();
                        startOffset = card.getTranslationX() - dx;
                        if (card.getParent() != null) card.getParent().requestDisallowInterceptTouchEvent(true);
                        android.view.MotionEvent cancel = android.view.MotionEvent.obtain(event);
                        cancel.setAction(android.view.MotionEvent.ACTION_CANCEL);
                        card.onTouchEvent(cancel);
                        cancel.recycle();
                        // 同一时间只有一条打开：开始滑这一条时先收回另一条。
                        if (clipboardSwipedCell != null && clipboardSwipedCell != this) closeClipboardSwipe();
                        showTrash(true);
                    }
                    card.setTranslationX(ClipboardSwipePolicy.offset(startOffset, dx, reveal));
                    return true;
                }
                case android.view.MotionEvent.ACTION_UP, android.view.MotionEvent.ACTION_CANCEL -> {
                    boolean handled = dragging;
                    if (dragging) {
                        float velocityX = 0f;
                        if (velocity != null && event.getActionMasked() == android.view.MotionEvent.ACTION_UP) {
                            track(event);
                            velocity.computeCurrentVelocity(1000);
                            velocityX = velocity.getXVelocity();
                        }
                        boolean open = ClipboardSwipePolicy.settlesOpen(card.getTranslationX(), velocityX, reveal, density);
                        if (open) {
                            clipboardSwipedText = text;
                            clipboardSwipedCell = this;
                        } else if (clipboardSwipedCell == this || text.equals(clipboardSwipedText)) {
                            clipboardSwipedText = null;
                            clipboardSwipedCell = null;
                        }
                        settle(open);
                    }
                    dragging = false;
                    cancelledPress = false;
                    recycleVelocity();
                    return handled;
                }
                default -> {
                    return dragging;
                }
            }
        }

        /** 把卡片动画到打开或收回的位置；收回的动画放完、卡片回到原位后再藏起删除按钮，动画里它还露在卡片右边。 */
        void settle(boolean open) {
            stopSettling();
            android.animation.ObjectAnimator animator = android.animation.ObjectAnimator.ofFloat(
                card, View.TRANSLATION_X, card.getTranslationX(), open ? -reveal : 0f);
            animator.setDuration(ClipboardSwipePolicy.SETTLE_MILLIS);
            if (!open) animator.addListener(new android.animation.AnimatorListenerAdapter() {
                private boolean cancelled;

                @Override public void onAnimationCancel(android.animation.Animator animation) {
                    cancelled = true;
                }

                @Override public void onAnimationEnd(android.animation.Animator animation) {
                    if (!cancelled) showTrash(false);
                }
            });
            settling = animator;
            animator.start();
        }

        /** 删除按钮只在卡片让开时显示：收着时 `INVISIBLE`，既不会在按压动画缩小卡片时从边上露出来，也点不到、读屏读不到。 */
        void showTrash(boolean shown) {
            if (shown) ViewPolicy.show(trash);
            else ViewPolicy.setInvisible(trash);
        }

        private void stopSettling() {
            if (settling != null) settling.cancel();
            settling = null;
        }

        // 速度按屏幕坐标算：卡片跟着手指移动，事件里相对卡片的坐标会把位移吃掉一部分。
        private void track(android.view.MotionEvent event) {
            if (velocity == null) return;
            android.view.MotionEvent screen = android.view.MotionEvent.obtain(event);
            screen.setLocation(event.getRawX(), event.getRawY());
            velocity.addMovement(screen);
            screen.recycle();
        }

        private void recycleVelocity() {
            if (velocity != null) velocity.recycle();
            velocity = null;
        }
    }

    /** 一格的宽度（像素），给双列时的删除区限宽用；面板还没量过时为 0，删除区按默认宽度。 */
    private float clipboardCellWidth(int columns) {
        int width = s.clipboardPanel.getWidth() - s.clipboardPanel.getPaddingLeft()
            - s.clipboardPanel.getPaddingRight();
        if (width <= 0) return 0f;
        int count = BoundsPolicy.atLeast(columns, 1);
        return (width - s.pixels(6) * (count - 1)) / (float) count;
    }

    /** 双列里的一格：等宽、与同一行的另一条同高，两格之间留 6 dp。 */
    private LinearLayout.LayoutParams clipboardCellParams(int column) {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1);
        if (column > 0) params.setMarginStart(s.pixels(6));
        return params;
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
        String message = CloudClipboardPanelPolicy.message(s.cloudClipboardStatus, s.cloudClipboardItems.size(),
            CloudClipboardRelayPolicy.webdav(s.cloudClipboardAccountId));
        if (!CloudClipboardPanelPolicy.showsItems(s.cloudClipboardStatus) || s.cloudClipboardItems.isEmpty()) {
            notes.add(clipboardNote(message));
            return;
        }
        long now = System.currentTimeMillis();
        int count = s.cloudClipboardItems.size();
        for (int index = 0; index < count; index++) {
            BackendAccount.ClipboardItem item = s.cloudClipboardItems.get(index);
            Button card = clipboardCard(index, count, item.text(),
                "云端 · " + RelativeTimePolicy.isoAgo(item.updatedAt(), now), false,
                () -> insertCloudClipboardText(item.text()));
            card.setContentDescription("点按插入云剪贴板记录，长按预览全文");
            card.setOnLongClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(card);
                clipboardPreview.show(item.text());
                return true;
            });
        }
    }

    /** 常用语面板的读取代次：面板关掉或重开后，迟到的结果直接丢弃。 */
    private long phraseGeneration;
    /** 没有常用语时的提示；它下面跟一个直达应用常用语页的「添加常用语」按钮。 */
    private static final String EMPTY_PHRASES = "还没有常用语";

    private String expandedPhraseCategory = "all";

    /** 工具栏「常用语」：在 `commonPhrasesWorker` 上读共享文档，每次打开采用保存的默认展开项。会话还在建时也能打开（#10086），读取不排在建会话的偏好读取和词库同步后面。 */
    void showCommonPhrases() {
        if (s.phraseScroll == null || s.phrasePanel == null) return;
        s.command(2);
        s.closeCandidatePanel();
        long generation = ++phraseGeneration;
        renderCommonPhrases(CommonPhrasesStore.Document.EMPTY, "正在读取常用语…");
        ViewPolicy.show(s.phraseScroll);
        try {
            s.commonPhrasesWorker.execute(() -> {
                CommonPhrasesStore.Result result;
                try { result = CommonPhrasesStore.load(s); }
                catch (RuntimeException | LinkageError error) { result = null; }
                CommonPhrasesStore.Document document = result != null && result.ok()
                    ? result.document() : CommonPhrasesStore.Document.EMPTY;
                String message = result == null ? "常用语读取失败" : result.ok() ? null : result.failure();
                s.main.post(() -> {
                    if (generation != phraseGeneration || !ViewPolicy.isVisible(s.phraseScroll)) return;
                    expandedPhraseCategory = document.expandedCategory();
                    renderCommonPhrases(document, message);
                });
            });
        } catch (RuntimeException error) {
            renderCommonPhrases(CommonPhrasesStore.Document.EMPTY, "常用语读取失败");
        }
    }

    private void renderCommonPhrases(CommonPhrasesStore.Document document, String message) {
        LinearLayout panel = s.phrasePanel;
        panel.removeAllViews();
        LayoutPolicy.setPaddingDp(panel, s, 8, 4, 8, 8);
        s.imeStyler.applySkinBackground(panel);
        if (message != null) {
            panel.addView(centeredNote(message, 14), LayoutPolicy.matchWidthWrapParams());
            s.imeStyler.applySkin();
            return;
        }
        LinearLayout toolbar = LayoutPolicy.row(s);
        TextView capacity = centeredNote("常用语 " + document.phrases().size() + " 条（自建 "
            + document.ownCount() + "/" + CommonPhrasesStore.MAX_OWN_PHRASES + "）", 12);
        toolbar.addView(capacity, LayoutPolicy.weightedWrapParams(1));
        String label = CommonPhrasesPanelPolicy.entryLabel(document.phrases().size());
        toolbar.addView(phraseButton(KeyboardKeyRole.RETURN, label, label, () -> {
            s.closeCommonPhrases();
            s.openHostPage("PHRASES");
        }), LayoutPolicy.wrapParams());
        panel.addView(toolbar, LayoutPolicy.matchWidthWrapParams());
        java.util.List<View> lines = new java.util.ArrayList<>();
        addPhraseCategory(document, "all", "全部", document.phrases(), lines);
        for (CommonPhrasesStore.Category category : document.categories()) {
            java.util.List<CommonPhrasesStore.Phrase> phrases = new java.util.ArrayList<>();
            for (CommonPhrasesStore.Phrase phrase : document.phrases())
                if (phrase.category().equals(category.id())) phrases.add(phrase);
            addPhraseCategory(document, category.id(), category.name(), phrases, lines);
        }
        s.imeStyler.applySkin();
        for (View hairline : lines) ViewPolicy.setBackgroundColor(hairline, Color.parseColor(s.skin.hairline()));
        ViewPolicy.setTextColor(capacity, ImeStyler.fade(s.skin.keyForeground(), .6));
    }

    private void addPhraseCategory(CommonPhrasesStore.Document document, String id, String name,
            java.util.List<CommonPhrasesStore.Phrase> phrases, java.util.List<View> lines) {
        boolean expanded = id.equals(expandedPhraseCategory);
        KeyboardPressButton heading = phraseButton(KeyboardKeyRole.PLAIN,
            (expanded ? "▾ " : "▸ ") + name + "（" + phrases.size() + "）",
            "常用语分类 " + name + (expanded ? "，已展开" : "，已收起"), () -> {
                expandedPhraseCategory = expanded ? "" : id;
                renderCommonPhrases(document, null);
            });
        ViewPolicy.setStartCenteredTextSizeSp(heading, 15);
        LayoutPolicy.setSymmetricPaddingDp(heading, s, 12, 8);
        s.phrasePanel.addView(heading, LayoutPolicy.matchWidthWrapParams());
        if (!expanded) return;
        if (phrases.isEmpty()) s.phrasePanel.addView(centeredNote(id.equals("all")
            ? EMPTY_PHRASES : "该分类还没有常用语", 14), LayoutPolicy.matchWidthWrapParams());
        for (CommonPhrasesStore.Phrase phrase : phrases) addCommonPhrase(phrase, lines);
    }

    private void addCommonPhrase(CommonPhrasesStore.Phrase phrase, java.util.List<View> lines) {
        LinearLayout panel = s.phrasePanel;
        String text = phrase.text();
        Runnable insert = () -> {
            if (s.connection == null) return;
            s.command(2);
            s.commitText(text);
            s.closeCommonPhrases();
            s.render();
        };
        String description = "常用语 " + (text.length() > 20 ? text.substring(0, 20) : text);
        KeyboardPressButton row = phraseButton(KeyboardKeyRole.PLAIN, text, description, insert);
        ViewPolicy.setStartCenteredTextSizeSp(row, 15);
        ViewPolicy.setMaxLinesEllipsized(row, 2);
        LayoutPolicy.setSymmetricPaddingDp(row, s, 12, 8);
        LinearLayout entry = LayoutPolicy.column(s);
        entry.addView(row, LayoutPolicy.matchWidthWrapParams());
        if (!phrase.note().isEmpty()) {
            TextView annotation = new TextView(s);
            annotation.setText("备注：" + phrase.note());
            annotation.setContentDescription("常用语备注：" + phrase.note());
            ViewPolicy.setTextSizeSp(annotation, 12);
            ViewPolicy.setMaxLinesEllipsized(annotation, 2);
            annotation.setAlpha(.65f);
            LayoutPolicy.setPaddingDp(annotation, s, 12, 0, 12, 8);
            // 备注区域也响应插入，但回调只捕获独立的正文。
            bindFeedbackAction(annotation, insert);
            entry.addView(annotation, LayoutPolicy.matchWidthWrapParams());
        }
        panel.addView(entry, LayoutPolicy.matchWidthWrapParams());
        View hairline = new View(s);
        LinearLayout.LayoutParams line = LayoutPolicy.matchWidthHeightPx(
            DimensionPolicy.atLeastOnePixel(s, 1));
        LayoutPolicy.setHorizontalMargins(line, s.pixels(12));
        panel.addView(hairline, line);
        lines.add(hairline);
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
        // 「更多」不经 closeToolbarPanels：会话在建时推迟的皮肤、输入方式在这里作废，用户回到键盘后它不再自己弹出来（#10086）。
        s.dropDeferredToolbarPanel();
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
        s.imeStickers.bindSymbolHeader(s.symbolPanel);
        s.imeStickers.load();
        symbolPreferences = s.getSharedPreferences(SYMBOL_RECENTS_PREFERENCES, Context.MODE_PRIVATE);
        ViewPolicy.hide(s.symbolPanel);
        s.keyboardSurface.addView(s.symbolPanel, LayoutPolicy.frameMatchParentParams());
    }

    /** 「常用」的使用记录：只存在本机，格式和表情的最近使用一样是一个 JSON 字符串数组。 */
    private List<String> loadSymbolRecents() {
        if (symbolPreferences == null) return List.of();
        String document = symbolPreferences.getString(SYMBOL_RECENTS_KEY, "[]");
        if (document == null || document.length() > 16_384) return List.of();
        try {
            JSONArray values = new JSONArray(document);
            int count = BoundsPolicy.atMost(values.length(), SymbolPanelModel.RECENTS_LIMIT * 2);
            ArrayList<String> stored = new ArrayList<>(count);
            for (int index = 0; index < count; index++) {
                Object value = values.opt(index);
                if (value instanceof String) stored.add((String) value);
            }
            return SymbolPanelModel.renderableRecents(SymbolPanelModel.normalizeRecents(stored),
                symbolRecentsGlyphPaint::hasGlyph);
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
                page = new SymbolCatalogPage(SymbolPanelModel.renderableCatalogItems(page.items(),
                    category.kaomoji(), s.emojiGlyphPaint::hasGlyph), page.nextOffset(), page.complete());
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
        long nextOffset = NumberPolicy.strictLong(value.opt("next_offset"), -1);
        Object complete = value.opt("complete");
        if (!(complete instanceof Boolean)
                || !SymbolPanelModel.validCatalogCursor(offset, items.size(), nextOffset, (Boolean) complete))
            throw new JSONException("Invalid symbol catalog cursor");
        return new SymbolCatalogPage(items, (int) nextOffset, (Boolean) complete);
    }

    void buildEmojiPanel() {
        s.emojiPreferences = s.getSharedPreferences(MSIMEInputService.EMOJI_RECENTS_PREFERENCES, Context.MODE_PRIVATE);
        s.emojiRecents = s.loadEmojiRecents();
        // 盖在键区上、不盖顶部工具栏；底栏空间由分类和删除键使用，返回入口沿用顶部工具栏。
        s.emojiPanel = LayoutPolicy.column(s);
        LayoutPolicy.setSymmetricPaddingDp(s.emojiPanel, s, 6, 4);
        ViewPolicy.setBackgroundColor(s.emojiPanel, Color.parseColor(s.skin.background()));
        s.emojiPanel.setContentDescription("表情面板");
        ViewPolicy.setFocusable(s.emojiPanel, true);
        s.emojiGrid = LayoutPolicy.column(s);
        s.emojiGridScroll = new EmojiCategoryScroll();
        s.emojiGridScroll.setFillViewport(false);
        s.emojiGridScroll.setVerticalScrollBarEnabled(false);
        s.emojiGridScroll.setContentDescription("表情网格；每行八个");
        s.emojiGridScroll.addView(s.emojiGrid, LayoutPolicy.scrollMatchWidthWrapParams());
        s.emojiGridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            // 表情包一次就全部摆好，滚动时只按位置给格子上图、放图。
            if (s.imeStickers.selected()) s.imeStickers.bindVisible();
            else if (scrollY > oldY && !view.canScrollVertically(1)) s.loadEmojiPage();
        });
        // 第一次排布出真实高度后按三行重排格子高度。
        s.emojiGridScroll.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            if ((bottom - top != oldBottom - oldTop || right - left != oldRight - oldLeft)
                    && s.emojiPickerVisible()) view.post(this::renderEmojiGrid);
        });
        s.emojiPanel.addView(s.emojiGridScroll, LayoutPolicy.weightedWidthParams(1));
        LinearLayout bar = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(bar);
        s.emojiTabs = LayoutPolicy.row(s);
        s.emojiTabs.setContentDescription("表情分类");
        LinearLayout.LayoutParams tabsParams = LayoutPolicy.weightedHeightPxParams(s.pixels(40), 1);
        LayoutPolicy.setHorizontalMargins(tabsParams, s.pixels(6));
        bar.addView(s.emojiTabs, tabsParams);
        Button deleteEmoji = MSIMEInputService.role(s.button(bar, "⌫", this::deleteFromEmojiPicker),
            KeyboardKeyRole.ACCENT);
        ViewPolicy.setTextSizeSp(deleteEmoji, 18);
        compactReplyControl(deleteEmoji, 0);
        deleteEmoji.setContentDescription("删除");
        deleteEmoji.setLayoutParams(LayoutPolicy.linearParams(s, 60, 40));
        s.emojiPanel.addView(bar, LayoutPolicy.matchWidthHeightPx(s.pixels(46)));
        ViewPolicy.hide(s.emojiPanel);
        s.keyboardSurface.addView(s.emojiPanel, LayoutPolicy.frameMatchParentParams());
        // 表情包清单和分类签缩略图提前在后台读好，打开面板时分类签已经在。
        s.imeStickers.load();
    }

    /** 横划在容器上接管并取消原按钮点击，纵向滚动和加载下一页仍由 ScrollView 处理。 */
    private final class EmojiCategoryScroll extends ScrollView {
        private final int touchSlop = ViewConfiguration.get(s).getScaledTouchSlop();
        private float downX;
        private float downY;
        private boolean horizontal;
        private boolean vertical;
        private boolean multiplePointers;

        EmojiCategoryScroll() { super(s); }

        @Override public boolean dispatchTouchEvent(MotionEvent event) {
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) {
                downX = event.getX();
                downY = event.getY();
                horizontal = vertical = multiplePointers = false;
            } else if (action == MotionEvent.ACTION_POINTER_DOWN) {
                multiplePointers = true;
            }
            float dx = event.getX() - downX;
            float dy = event.getY() - downY;
            if (action == MotionEvent.ACTION_MOVE && !horizontal && !vertical && !multiplePointers) {
                if (Math.abs(dx) > touchSlop && Math.abs(dx) > Math.abs(dy) * 1.5f) {
                    horizontal = true;
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    super.dispatchTouchEvent(cancel);
                    cancel.recycle();
                } else if (Math.abs(dy) > touchSlop) {
                    vertical = true;
                }
            }
            if (!horizontal) return super.dispatchTouchEvent(event);
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                horizontal = false;
                if (action == MotionEvent.ACTION_UP && !multiplePointers &&
                        Math.abs(dx) >= s.pixels(48) && Math.abs(dx) > Math.abs(dy) * 1.5f) {
                    // 按底栏分类签的顺序切换，常用在最左，两端不回绕。
                    int category = s.imeStickers.swipeCategory(dx < 0 ? 1 : -1);
                    if (category != StickerPackModel.NO_CATEGORY) s.selectEmojiCategory(category);
                }
            }
            return true;
        }
    }
}
