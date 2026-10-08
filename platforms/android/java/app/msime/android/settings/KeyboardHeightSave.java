package app.msime.android;

import java.io.IOException;
import java.util.function.Supplier;

/**
 * 键盘里调整布局后的落盘顺序。
 *
 * <p>键盘高度只存在 Android 本地设置里（`platform.android.keyboard_height_adjustment`），按键间距、行间距和语音入口在共享偏好文档里，两者分开写：先写本地高度，再按 revision 比较并交换写共享文档。原来高度要等共享文档写成功才写，共享文档一冲突（设置页或同步刚写过）或写入失败，用户刚调好的高度就跟着丢掉，下次弹出键盘又回到设置页里的旧值。
 */
public final class KeyboardHeightSave {
    /** 写本地高度；写不成时抛出。 */
    public interface HeightWriter {
        void write() throws IOException;
    }

    /**
     * 一次保存的结果。
     *
     * @param heightSaved 本地高度写成了（这次不需要写高度时也为真）
     * @param sharedAttempted 这次写了共享文档
     * @param sharedResponse 共享文档保存的原始响应；没写或写入时出错为 null
     */
    public record Result(boolean heightSaved, boolean sharedAttempted, String sharedResponse) {}

    private KeyboardHeightSave() {}

    /**
     * 先写高度（`height` 为 null 表示高度没变、不写），再写共享文档（`shared` 为 null 表示间距和语音入口没变、不写）。两步互不依赖：高度写不成不妨碍共享文档，共享文档失败也不撤回已写下的高度。在后台线程调用。
     */
    public static Result run(HeightWriter height, Supplier<String> shared) {
        boolean heightSaved = true;
        if (height != null) {
            try {
                height.write();
            } catch (IOException | IllegalArgumentException error) {
                heightSaved = false;
            }
        }
        String response = shared == null ? null : shared.get();
        return new Result(heightSaved, shared != null, response);
    }
}
