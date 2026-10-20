package app.msime.android;

import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.ImageDecoder;
import android.graphics.Paint;
import android.graphics.RectF;
import android.net.Uri;
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
 * 表情面板最后一个分类「水杉娘」表情包：读随包清单、按格子大小解码缩略图、点一张发出去。网格和分类签由 {@link ImePanels} 搭，清单校验和文案在 {@link StickerPackModel}。
 *
 * <p>直接从 APK 的 assets 读（`build-apk.sh` 把 `resources/stickers/shuishan` 放到 `assets/stickers/shuishan`），不解包到 filesDir。清单、缩略图、PNG 编码和清理都在 {@link #worker} 上，主线程只摆格子。缩略图只给看得见的格子（上下各多一行），其余格子放掉位图；解码结果按字节数有界缓存，离开这个分类或收起面板时清空。
 *
 * <p>发送：输入框声明收 PNG 时把 WebP 转成保留透明的 PNG，写进 `cache/sticker-images/`，经本应用的 FileProvider（`${applicationId}.files`）给出 content URI，用 `commitContent` 带读权限交给输入框；不收或没收下时把同一个 URI 放进系统剪贴板，提示用户长按粘贴。目录里只留最近一张，发出 {@link ClipboardImagePolicy#RETENTION_MS} 后、键盘进程启动和退出时都清空，做法与「最近复制」的图片副本（{@link ClipboardImageCache}）相同。
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
    /** 发出 {@link ClipboardImagePolicy#RETENTION_MS} 之后删掉缓存里的 PNG。 */
    private final Runnable cleanup = this::clearSentImages;
    private StickerPackModel.Pack pack;
    private boolean loadStarted;
    private Bitmap tabIcon;
    private List<StickerButton> cells = List.of();
    private int rowHeight;
    private int columns = StickerPackModel.MIN_COLUMNS;
    private int thumbnailEdge;
    /** 换了格子大小、离开分类或收起面板时加一：之前排队的解码结果不再用。 */
    private long thumbnailGeneration;

    ImeStickers(MSIMEInputService s) {
        this.s = s;
    }

    /** 在工作线程上读一次清单和分类签上的缩略图；读完时面板开着就重画分类签。读不出来时这个分类不出现，同一个进程里不再重试（随包资源坏了重试也一样）。 */
    void load() {
        if (loadStarted || worker.isShutdown()) return;
        loadStarted = true;
        int iconEdge = s.pixels(StickerPackModel.TAB_ICON_DP);
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
            StickerPackModel.Pack result = loaded;
            Bitmap resultIcon = icon;
            s.main.post(() -> {
                if (worker.isShutdown() || result == null) return;
                pack = result;
                tabIcon = resultIcon;
                if (s.emojiPickerVisible()) {
                    s.imePanels.renderEmojiTabs();
                    s.imeStyler.applySkin();
                }
            });
        });
    }

    boolean available() { return pack != null; }

    StickerPackModel.Pack pack() { return pack; }

    Bitmap tabIcon() { return tabIcon; }

    /** 表情包在分类序号里的位置：排在所有 Emoji 分类之后。 */
    int category() { return EmojiCatalogModel.categories().size(); }

    /** 横划和点分类签能到的最后一个分类序号。 */
    int lastEmojiCategory() { return available() ? category() : EmojiCatalogModel.categories().size() - 1; }

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

    /** 离开这个分类或收起面板：放掉格子和缩略图，内存里只留分类签那一张。 */
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
            s.main.post(() -> deliver(generation, current, sticker, result, commit));
        });
    }

    /** {@link #send} 在主线程上的后半段：输入框没变时经 FileProvider 交给它，交不出去就复制到剪贴板，并安排过一会儿删掉 PNG。 */
    private void deliver(long generation, StickerPackModel.Pack current, StickerPackModel.Sticker sticker,
            File png, boolean commit) {
        if (worker.isShutdown()) return;
        if (generation != s.clipboardImageGeneration || s.connection == null) {
            // 编码期间换了输入框、收起了键盘或重建了会话（包括切换隐私模式）：这张不再发，PNG 也不留。
            clearSentImages();
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
            clearSentImages();
            s.notice("无法发送这张表情");
            return;
        }
        s.main.removeCallbacks(cleanup);
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
        if (!sent) {
            if (!copy(uri, sticker)) {
                clearSentImages();
                s.notice("无法复制这张表情");
                return;
            }
            s.notice(StickerPackModel.copiedNotice(current, sticker));
        }
        s.main.postDelayed(cleanup, ClipboardImagePolicy.RETENTION_MS);
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

    /** 在 {@link #worker} 上清空发出去的 PNG；键盘进程正在退出、线程已经停了时什么也不做（退出前已经排过一次清理）。 */
    void clearSentImages() {
        if (worker.isShutdown()) return;
        File directory = new File(s.getCacheDir(), StickerPackModel.DIRECTORY);
        worker.execute(() -> ClipboardImageCache.clear(directory));
    }

    /** 键盘进程退出：清理排在 shutdown 之前，正在写的那张写完后连同更早的一起删掉。 */
    void shutdown() {
        s.main.removeCallbacks(cleanup);
        clearSentImages();
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
