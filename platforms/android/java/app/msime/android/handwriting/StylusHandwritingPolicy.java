package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 在应用的输入框上直接用手写笔书写（Android 14 的输入法手写笔接口，#7080）的规则：接不接这次书写、停笔多久后识别、屏幕上的笔迹怎样变成交给识别器的请求、模型没就绪时说什么。不依赖 Android，由 `StylusHandwritingPolicySmoke` 覆盖。
 *
 * <p>系统在用户拿笔在输入框上写字时问输入法要不要接；接了以后笔的事件全部交给输入法，直到输入法结束会话或笔停下超过 {@link #SESSION_IDLE_TIMEOUT_MILLIS}。键盘不会因此弹出，所以识别结果直接上屏第一候选，没有候选栏可选。
 */
public final class StylusHandwritingPolicy {
    /** 笔停下这么久后系统结束书写会话，笔的事件回到应用（系统默认 10 秒，期间拿笔点不了别处）。必须长于最长的识别等待时间。 */
    public static final long SESSION_IDLE_TIMEOUT_MILLIS = 3000;
    /** 模型没就绪时提示停留多久再结束会话。 */
    public static final long HINT_MILLIS = 2500;
    /** 模型还在检查时，隔多久再看一次能不能识别。 */
    public static final long AVAILABILITY_POLL_MILLIS = 250;
    /** 到了该识别的时候模型还在检查，最多再等这么久，之后丢掉笔迹并提示还在检查，不让笔迹无声无息地留着。加上最长的识别等待时间仍短于 {@link #SESSION_IDLE_TIMEOUT_MILLIS}，提示出来时会话还在。 */
    public static final long CHECK_TIMEOUT_MILLIS = 1000;
    /** 交给识别器的书写区域最多这么大，与 {@link HandwritingRecognizer.Request} 的上限一致。 */
    static final float MAX_AREA = 4096;

    private StylusHandwritingPolicy() {}

    /** 系统问要不要接这次书写时的回答。 */
    public enum Decision {
        /** 接下，记笔迹并识别。 */
        ACCEPT,
        /** 接下，但只在落笔处显示 {@link #hint} 的提示，不记笔迹，过一会儿结束会话。 */
        HINT,
        /** 不接，笔照常交给应用。 */
        DECLINE
    }

    /**
     * @param editionOffersHandwriting 本版本有没有手写（日文、越南文、藏文版没有中文手写模型）
     * @param engineEditor 输入框是否走引擎；密码、数字这类不走引擎的输入框不接
     * @param chineseInput 当前是否在输入中文；专用英文、日韩越藏方案和本地模式下不接，手写模型只认汉字
     * @param availability 手写笔用的识别器此刻的状态
     * @param hintShown 这个输入框里是否已经提示过模型没就绪；提示过就不再接，免得每一笔都被拦下
     */
    public static Decision start(boolean editionOffersHandwriting, boolean engineEditor, boolean chineseInput,
                                 HandwritingRecognizer.Availability availability, boolean hintShown) {
        if (!editionOffersHandwriting || !engineEditor || !chineseInput || availability == null) {
            return Decision.DECLINE;
        }
        return switch (availability) {
            // 「正在检查」是识别器刚建好、还没查完模型在不在：先接下记笔迹，识别时再看。
            case READY, DOWNLOADING -> Decision.ACCEPT;
            case DOWNLOAD_REQUIRED, UNAVAILABLE -> hintShown ? Decision.DECLINE : Decision.HINT;
        };
    }

    /** 模型没就绪时的提示，用词与键盘里手写面板的状态行一致。 */
    public static String hint(HandwritingRecognizer.Availability availability) {
        return switch (availability) {
            case UNAVAILABLE -> "此构建不含手写识别";
            case DOWNLOADING -> "正在检查中文手写模型，请稍后再写";
            default -> "首次下载后可离线手写：请在键盘的手写面板里下载中文手写模型";
        };
    }

    /**
     * 停笔后等多久识别：与手写面板相同，服务的防抖 {@code serviceDebounce} 加上「识别等待时间」比它多出的部分。笔画已到 {@link HandwritingInk#MAX_STROKES} 时立即识别，再写也记不下了。
     */
    public static long recognitionDelayMillis(HandwritingPreferences preferences, long serviceDebounce,
                                              int strokeCount) {
        if (strokeCount >= HandwritingInk.MAX_STROKES) return 0;
        return BoundsPolicy.nonNegative(serviceDebounce) + preferences.extraDelay(serviceDebounce);
    }

    /**
     * 把屏幕坐标下的笔迹变成识别请求：书写窗口铺满整个屏幕，笔迹只占其中一小块，直接把屏幕大小当书写区域，识别器会把一行字当成一张大纸角落里的小涂鸦。所以先裁到笔迹的外接框，四周各留 {@code padding}，宽高都不小于 {@code minimumSide}（单写一个「一」时外接框几乎没有高度），笔迹居中；超出 {@link #MAX_AREA} 时整体等比缩小。笔画顺序和时间戳不变。
     *
     * @return 请求；没有笔迹时为 null
     */
    public static HandwritingRecognizer.Request request(long revision, List<List<HandwritingInk.Point>> strokes,
                                                        float padding, float minimumSide) {
        if (strokes == null || strokes.isEmpty()) return null;
        float minX = Float.MAX_VALUE;
        float minY = Float.MAX_VALUE;
        float maxX = 0;
        float maxY = 0;
        for (List<HandwritingInk.Point> stroke : strokes) {
            for (HandwritingInk.Point point : stroke) {
                minX = Math.min(minX, point.x());
                minY = Math.min(minY, point.y());
                maxX = Math.max(maxX, point.x());
                maxY = Math.max(maxY, point.y());
            }
        }
        if (minX > maxX) return null;
        float margin = BoundsPolicy.nonNegative(padding);
        float inkWidth = maxX - minX;
        float inkHeight = maxY - minY;
        float width = Math.max(Math.max(inkWidth + 2 * margin, minimumSide), 1);
        float height = Math.max(Math.max(inkHeight + 2 * margin, minimumSide), 1);
        float offsetX = (width - inkWidth) / 2 - minX;
        float offsetY = (height - inkHeight) / 2 - minY;
        float scale = Math.min(1, Math.min(MAX_AREA / width, MAX_AREA / height));
        float areaWidth = Math.min(width * scale, MAX_AREA);
        float areaHeight = Math.min(height * scale, MAX_AREA);
        List<List<HandwritingInk.Point>> converted = new ArrayList<>(strokes.size());
        for (List<HandwritingInk.Point> stroke : strokes) {
            List<HandwritingInk.Point> points = new ArrayList<>(stroke.size());
            for (HandwritingInk.Point point : stroke) {
                points.add(new HandwritingInk.Point(
                    BoundsPolicy.bounded((point.x() + offsetX) * scale, 0, areaWidth),
                    BoundsPolicy.bounded((point.y() + offsetY) * scale, 0, areaHeight),
                    point.timeMillis()));
            }
            converted.add(points);
        }
        return new HandwritingRecognizer.Request(revision, converted, areaWidth, areaHeight);
    }
}
