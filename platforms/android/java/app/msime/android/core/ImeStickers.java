package app.msime.android;

import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.ImageDecoder;
import android.graphics.Paint;
import android.graphics.RectF;
import android.net.Uri;
import android.os.Bundle;
import android.util.LruCache;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputContentInfo;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONException;

/**
 * 表情面板的「水杉娘」表情包：读随包清单、按格子大小解码缩略图、点一张发出去。网格和分类签由 {@link ImePanels} 搭，清单校验和文案在 {@link StickerPackModel}。
 *
 * <p>直接从 APK 的 assets 读（`build-apk.sh` 把 `resources/stickers/shuishan` 放到 `assets/stickers/shuishan`），不解包到 filesDir。清单、缩略图、PNG 编码和清理都在 {@link #worker} 上，主线程只摆格子。缩略图只给看得见的格子（上下各多一行），其余格子放掉位图；解码结果按字节数有界缓存，离开这个分类或收起面板时清空。
 *
 * <p>发送：输入框声明收 PNG 时把 WebP 转成保留透明的 PNG，写进 `cache/sticker-images/`，经本应用的 FileProvider（`${applicationId}.files`）给出 content URI，用 `commitContent` 带读权限交给输入框；不收或没收下时把同一个 URI 放进系统剪贴板，提示用户长按粘贴。目录里始终至多一张：写下一张之前删掉旧的（{@link ClipboardImageCache#replace}），此外不定时删、键盘进程启动和退出时也不清空。贴纸是随包素材，不是用户内容，没有尽快删除的隐私理由；而复制到剪贴板的 URI 要在用户切到别的应用、键盘进程被回收之后仍能粘贴，所以不沿用「最近复制」图片副本的 {@link ClipboardImagePolicy#RETENTION_MS}。
 */
final class ImeStickers {
    /** 缩略图缓存的上限：每格不超过 160 px 时 66 张整套放得下（约 6.5 MiB），格子更大时只留最近画过的那些。 */
    private static final int THUMBNAIL_CACHE_BYTES = 7 * 1024 * 1024;

    private final MSIMEInputService s;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final LruCache<Integer, Bitmap> thumbnails = new LruCache<>(THUMBNAIL_CACHE_BYTES) {
        @Override protected int sizeOf(Integer key, Bitmap value) { return value.getAllocationByteCount(); }
    };
    private final HashSet<Integer> pending = new HashSet<>();
    /** 解码失败过的格子：滚动时不再一遍遍重试，换格子大小或重新打开时才再试。 */
    private final HashSet<Integer> failed = new HashSet<>();
    private StickerPackModel.Pack pack;
    private boolean loadStarted;
    private Bitmap tabIcon;
    private Bitmap symbolReturnLeft;
    private Bitmap symbolReturnRight;
    private List<StickerButton> cells = List.of();
    private int rowHeight;
    private int columns = StickerPackModel.MIN_COLUMNS;
    private int thumbnailEdge;
    /** 换了格子大小、离开分类或收起面板时加一：之前排队的解码结果不再用。 */
    private long thumbnailGeneration;

    ImeStickers(MSIMEInputService s) {
        this.s = s;
    }

    /** 在工作线程上读一次清单和分类签上的缩略图；读完时面板开着就重画分类签，不改当前分类。读不出来时只省略水杉娘分类，常用仍在最左；同一进程里不重试随包的坏资源。 */
    void load() {
        if (loadStarted || worker.isShutdown()) return;
        loadStarted = true;
        // 分类签和语音面板共用这一张有界缩略图，避免放大分类签的小图时出现锯齿。
        int iconEdge = s.pixels((int) Math.max(StickerPackModel.TAB_ICON_DP, VoiceListeningView.ORB_DP));
        worker.execute(() -> {
            StickerPackModel.Pack loaded = null;
            Bitmap icon = null;
            try (InputStream input = s.getAssets().open(
                    StickerPackModel.ASSET_DIRECTORY + "/" + StickerPackModel.MANIFEST)) {
                loaded = StickerPackModel.decode(new String(
                    HttpBodyPolicy.readRequired(input, StickerPackModel.MAX_MANIFEST_BYTES), StandardCharsets.UTF_8));
            } catch (IOException | JSONException error) {
                // 没打包（Tauri 合包不带）或清单不合规：不显示这个分类，不记录内容。
            }
            if (loaded != null) {
                try {
                    icon = decode(loaded.stickers().get(0), iconEdge);
                } catch (IOException | RuntimeException error) {
                    // 解码器在系统边界上抛的异常：分类签照样出现，只是没有图。
                }
            }
            Bitmap returnLeft = decodeHeader("left");
            Bitmap returnRight = decodeHeader("right");
            StickerPackModel.Pack result = loaded;
            Bitmap resultIcon = icon;
            s.main.post(() -> {
                if (worker.isShutdown()) return;
                symbolReturnLeft = returnLeft;
                symbolReturnRight = returnRight;
                if (s.symbolPanel != null) bindSymbolHeader(s.symbolPanel);
                if (result == null) return;
                pack = result;
                tabIcon = resultIcon;
                if (s.emojiPickerVisible()) {
                    s.imePanels.renderEmojiTabs();
                    s.imeStyler.applySkin();
                }
            });
        });
    }

    /** 随包插图在已有工作线程读取，不依赖表情包清单是否可用。 */
    private Bitmap decodeHeader(String side) {
        try (InputStream input = s.getAssets().open("symbol-return/" + side + ".webp")) {
            return BitmapFactory.decodeStream(input);
        } catch (IOException | RuntimeException error) {
            // 资源损坏时保留完整文字按钮，不记录图像或用户内容。
            return null;
        }
    }

    void bindSymbolHeader(SymbolPanelView panel) {
        panel.setHeaderMascots(symbolReturnLeft, symbolReturnRight);
    }

    boolean available() { return pack != null; }

    StickerPackModel.Pack pack() { return pack; }

    Bitmap tabIcon() { return tabIcon; }

    /** 表情包的分类序号：取在所有 Emoji 分类之后的那个号，显示顺序见 {@link StickerPackModel#categoryOrder}。 */
    int category() { return StickerPackModel.category(EmojiCatalogModel.categories().size()); }

    /** 点分类签能选到的最大分类序号。 */
    int lastEmojiCategory() { return available() ? category() : EmojiCatalogModel.categories().size() - 1; }

    /** 底栏分类签从左到右的分类序号，横划按同一顺序切换。 */
    List<Integer> categoryOrder() {
        return StickerPackModel.categoryOrder(available(), EmojiCatalogModel.categories().size());
    }

    /** 从当前分类横划一步（`step` 为 1 往右、-1 往左）到的分类；到头时是 {@link StickerPackModel#NO_CATEGORY}。 */
    int swipeCategory(int step) {
        return StickerPackModel.swipeCategory(s.emojiSelectedCategory, step, available(),
            EmojiCatalogModel.categories().size());
    }

    boolean selected() { return available() && s.emojiSelectedCategory == category(); }

    /** 网格刚搭好：记下格子和每格能画的边长，边长变了就丢掉按旧边长解码的缩略图，再给看得见的格子上图。 */
    void bindGrid(List<StickerButton> nextCells, int nextColumns, int nextRowHeight, int edge) {
        cells = List.copyOf(nextCells);
        columns = Math.max(1, nextColumns);
        rowHeight = Math.max(1, nextRowHeight);
        if (edge != thumbnailEdge) {
            thumbnailEdge = edge;
            discardThumbnails();
        }
        bindVisible();
        if (s.emojiGridScroll != null) s.emojiGridScroll.post(this::bindVisible);
    }

    /** 按网格的滚动位置给看得见的格子（上下各多一行）上图，其余格子放掉位图。 */
    void bindVisible() {
        if (!selected() || cells.isEmpty() || s.emojiGridScroll == null) return;
        int viewport = s.emojiGridScroll.getHeight();
        if (viewport <= 0) viewport = rowHeight * 3;
        int top = s.emojiGridScroll.getScrollY() - rowHeight;
        int bottom = s.emojiGridScroll.getScrollY() + viewport + rowHeight;
        for (int index = 0; index < cells.size(); index++) {
            int rowTop = index / columns * rowHeight;
            StickerButton cell = cells.get(index);
            if (rowTop + rowHeight <= top || rowTop >= bottom) {
                cell.setImage(null);
                continue;
            }
            Bitmap thumbnail = thumbnails.get(index);
            if (thumbnail != null) cell.setImage(thumbnail);
            else if (!cell.hasImage()) requestThumbnail(index);
        }
    }

    private void requestThumbnail(int index) {
        if (worker.isShutdown() || failed.contains(index) || !pending.add(index)) return;
        StickerPackModel.Sticker sticker = pack.stickers().get(index);
        long generation = thumbnailGeneration;
        int edge = thumbnailEdge;
        worker.execute(() -> {
            Bitmap decoded = null;
            try {
                decoded = decode(sticker, edge);
            } catch (IOException | RuntimeException error) {
                // 解码器在系统边界上抛的异常：这一格留空，不影响别的格子和发送。
            }
            Bitmap result = decoded;
            s.main.post(() -> {
                if (generation != thumbnailGeneration || worker.isShutdown()) return;
                pending.remove(index);
                if (result == null) {
                    failed.add(index);
                    return;
                }
                thumbnails.put(index, result);
                bindVisible();
            });
        });
    }

    /** 离开这个分类或收起面板：放掉格子和缩略图，保留分类签与返回栏的三张固定小图。 */
    void release() {
        cells = List.of();
        discardThumbnails();
    }

    private void discardThumbnails() {
        thumbnailGeneration++;
        pending.clear();
        failed.clear();
        thumbnails.evictAll();
    }

    /** 当前输入框收 PNG 就直接发，否则放进系统剪贴板；先结束正在拼写的内容。不记进表情的「最近」。 */
    void send(int index) {
        if (!selected() || !s.emojiPickerVisible() || s.connection == null
                || index < 0 || index >= pack.stickers().size() || worker.isShutdown()) return;
        // 和打开表情面板时一样先把组字按原样上屏，图片才不会插在拼写中间。
        s.command(2);
        if (s.connection == null) return;
        StickerPackModel.Pack current = pack;
        StickerPackModel.Sticker sticker = current.stickers().get(index);
        EditorInfo info = s.getCurrentInputEditorInfo();
        boolean commit = info != null && StickerPackModel.commitsContent(info.contentMimeTypes);
        // 微信这类经私有命令收表情的输入框：记下它的包名，交图前给它读权限。
        String expressionTarget = info != null && info.extras != null && info.packageName != null
            && StickerPackModel.sendsExpression(info.contentMimeTypes,
                info.extras.getInt(StickerPackModel.EXPRESSION_EXTRA, 0)) ? info.packageName : null;
        long generation = s.clipboardImageGeneration;
        File directory = new File(s.getCacheDir(), StickerPackModel.DIRECTORY);
        worker.execute(() -> {
            File png = null;
            try {
                png = writePng(sticker, directory);
            } catch (IOException | RuntimeException error) {
                // 解码、编码或写缓存失败：下面只提示，不留半成品（ClipboardImageCache.replace 已删掉）。
            }
            File result = png;
            s.main.post(() -> deliver(generation, current, sticker, result, commit, expressionTarget));
        });
    }

    /** {@link #send} 在主线程上的后半段：输入框没变时经 FileProvider 交给它（`commitContent` 或微信的私有命令），交不出去就复制到剪贴板。PNG 留到写下一张时才被替换。 */
    private void deliver(long generation, StickerPackModel.Pack current, StickerPackModel.Sticker sticker,
            File png, boolean commit, String expressionTarget) {
        if (worker.isShutdown()) return;
        if (generation != s.clipboardImageGeneration || s.connection == null) {
            // 编码期间换了输入框、收起了键盘或重建了会话（包括切换隐私模式）：这张不再发。
            return;
        }
        if (png == null) {
            s.notice("无法发送这张表情");
            return;
        }
        Uri uri;
        try {
            uri = ClipboardImageCache.contentUri(s, png);
        } catch (IOException error) {
            s.notice("无法发送这张表情");
            return;
        }
        boolean sent = false;
        if (commit) {
            try {
                InputContentInfo content = new InputContentInfo(uri,
                    new ClipDescription(sticker.name(), new String[] {StickerPackModel.MIME_TYPE}));
                sent = InputConnectionGuard.commitContent(s.connection, content,
                    InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION, null);
            } catch (IllegalArgumentException error) {
                sent = false;
            }
        }
        if (!sent && expressionTarget != null) sent = sendExpression(uri, expressionTarget);
        if (!sent) {
            if (!copy(uri, sticker)) {
                s.notice("无法复制这张表情");
                return;
            }
            s.notice(StickerPackModel.copiedNotice(current, sticker));
        }
    }

    /**
     * 经微信的私有表情命令交图：先把 URI 的读权限授给输入框所在的应用，再发命令。微信收下后自己弹出发送确认，返回 true；返回 false 或授权失败时由调用方退回剪贴板。
     *
     * <p>授权在 `commitContent` 里由系统随 `INPUT_CONTENT_GRANT_READ_URI_PERMISSION` 完成，私有命令没有这一步，只能经 `grantUriPermission` 显式授给对方包名（provider 声明了 `grantUriPermissions`）。
     */
    private boolean sendExpression(Uri uri, String target) {
        try {
            s.grantUriPermission(target, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
        } catch (RuntimeException error) {
            // 系统边界上的拒绝（对方包不可见、provider 不允许授权）：退回剪贴板。
            return false;
        }
        Bundle data = new Bundle();
        data.putParcelable(StickerPackModel.EXPRESSION_URI, uri);
        return InputConnectionGuard.performPrivateCommand(s.connection, StickerPackModel.EXPRESSION_COMMAND, data);
    }

    /**
     * 把 PNG 的 content URI 放进系统剪贴板。描述直接写 `image/png`，不用 `ClipData.newUri`：那样要在主线程上经 binder 问 FileProvider 类型，主进程不在时还得先把它拉起来。
     *
     * <p>系统把读权限授给之后读剪贴板的应用（provider 声明了 `grantUriPermissions`）。键盘自己的剪贴板监听也会收到这一次变化，{@link #ownsClip} 让它不把贴纸当成用户复制的图片。
     */
    private boolean copy(Uri uri, StickerPackModel.Sticker sticker) {
        ClipboardManager manager = s.getSystemService(ClipboardManager.class);
        if (manager == null) return false;
        try {
            manager.setPrimaryClip(new ClipData(
                new ClipDescription(sticker.name(), new String[] {StickerPackModel.MIME_TYPE}),
                new ClipData.Item(uri)));
            return true;
        } catch (RuntimeException error) {
            // 剪贴板服务在系统边界上的拒绝（SecurityException 等）。
            return false;
        }
    }

    /** 剪贴板里的这张图是不是键盘自己放进去的贴纸。 */
    boolean ownsClip(Uri uri) {
        return uri != null && StickerPackModel.ownClipUri(
            uri.getScheme(), uri.getAuthority(), uri.getPath(), s.getPackageName());
    }

    /** 键盘进程退出：停掉工作线程、放掉缩略图。发送目录里那一张留着，剪贴板里的 URI 在进程被回收后仍要能粘贴。 */
    void shutdown() {
        worker.shutdown();
        release();
    }

    /** 按 `edge` 等比解码一张贴纸（不放大），软件位图：既要画进键盘，也要能编码成 PNG。 */
    private Bitmap decode(StickerPackModel.Sticker sticker, int edge) throws IOException {
        ImageDecoder.Source source = ImageDecoder.createSource(s.getAssets(), StickerPackModel.assetPath(sticker));
        return ImageDecoder.decodeBitmap(source, (decoder, info, ignored) -> {
            decoder.setAllocator(ImageDecoder.ALLOCATOR_SOFTWARE);
            int[] size = StickerPackModel.fit(info.getSize().getWidth(), info.getSize().getHeight(), edge);
            decoder.setTargetSize(size[0], size[1]);
        });
    }

    /** 原尺寸解码后编码成 PNG（保留透明），写进只留一张的发送目录。 */
    private File writePng(StickerPackModel.Sticker sticker, File directory) throws IOException {
        Bitmap bitmap = decode(sticker, StickerPackModel.MAX_SOURCE_EDGE);
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new IOException("sticker not encoded");
        } finally {
            bitmap.recycle();
        }
        return ClipboardImageCache.replace(new ByteArrayInputStream(output.toByteArray()), directory,
            StickerPackModel.EXTENSION, ClipboardImagePolicy.MAX_BYTES);
    }

    /** 画一张贴纸的按键：沿用表情格的按压反馈（{@link KeyboardPressButton}），位图等比缩放后居中，四周留出 `inset`，`maxEdge` 大于 0 时再限制边长（分类签用）。 */
    static final class StickerButton extends KeyboardPressButton {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
        private final RectF bounds = new RectF();
        private Bitmap image;
        private final int inset;
        private final int maxEdge;

        StickerButton(Context context, int inset, int maxEdge) {
            super(context);
            this.inset = inset;
            this.maxEdge = maxEdge;
            ViewPolicy.setAllCapsFalse(this);
        }

        boolean hasImage() { return image != null; }

        void setImage(Bitmap next) {
            if (next == image) return;
            image = next;
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            if (image == null || image.isRecycled()) return;
            float available = Math.min(getWidth(), getHeight()) - 2f * inset;
            if (maxEdge > 0) available = Math.min(available, maxEdge);
            if (available <= 0) return;
            float scale = Math.min(available / image.getWidth(), available / image.getHeight());
            float width = image.getWidth() * scale;
            float height = image.getHeight() * scale;
            float left = (getWidth() - width) / 2f;
            float top = (getHeight() - height) / 2f;
            bounds.set(left, top, left + width, top + height);
            canvas.drawBitmap(image, null, bounds, paint);
        }
    }
}
