package app.msime.android;

import java.util.Locale;

/**
 * 设置页「本地模型」组的规则：哪些模型列出来、每一行显示什么、该给哪个操作。下载状态由 `ResourcePackService` 记着，这里只把它和安装、选用状态合成一行。
 */
public final class LocalVoiceModelPolicy {
    /** 一行的操作：下载、取消下载、校验中（不能点）、重试、管理（已装，点开选用或删除）。 */
    public enum Action { DOWNLOAD, CANCEL, VERIFYING, RETRY, MANAGE }

    private LocalVoiceModelPolicy() {}

    /** 目录标了只给桌面用的模型（体积或算力不适合手机）不列出，与 HarmonyOS 相同；已经装上的照样列出，免得删不掉。 */
    public static boolean visible(boolean desktopOnly, boolean installed) {
        return !desktopOnly || installed;
    }

    /** 下载服务里这个模型的阶段（{@code ResourcePackService.Phase} 加上「没在下」），设置页换算后传进来；这个类不依赖服务，好在 JVM 冒烟里测。 */
    public enum Download { NONE, DOWNLOADING, VERIFYING, FAILED }

    /** 这一行该给的操作：下载中和校验中优先于已安装（重新下载时旧模型还在）。 */
    public static Action action(boolean installed, Download download) {
        if (download == Download.DOWNLOADING) return Action.CANCEL;
        if (download == Download.VERIFYING) return Action.VERIFYING;
        if (installed) return Action.MANAGE;
        return download == Download.FAILED ? Action.RETRY : Action.DOWNLOAD;
    }

    /** 已装模型行尾的值：正在用的写「使用中」，其余「已下载」。 */
    public static String installedValue(boolean selected) {
        return selected ? "使用中" : "已下载";
    }

    /** 没装的模型的说明：目录里的简介，加上下载大小和运行时大约占用的内存。 */
    public static String summary(String description, long archiveBytes, String memory) {
        StringBuilder text = new StringBuilder(description == null ? "" : description.trim());
        if (archiveBytes > 0) {
            if (text.length() > 0) text.append(" · ");
            text.append("需下载 ").append(String.format(Locale.ROOT, "%.0f MB", archiveBytes / 1_000_000.0));
        }
        if (memory != null && !memory.trim().isEmpty()) {
            if (text.length() > 0) text.append(" · ");
            text.append("运行时").append(memory.trim());
        }
        return text.toString();
    }

    /** 模型目录是否就是偏好里选用的那个；比较前去掉首尾空白，两边都空不算选中。 */
    public static boolean selected(String modelPath, String chosenPath) {
        String model = modelPath == null ? "" : modelPath.trim();
        return !model.isEmpty() && model.equals(chosenPath == null ? "" : chosenPath.trim());
    }

    /** 删除成功后按工作线程重新读到的当前路径更新选择；删除旧模型不能清掉用户后来选的模型。 */
    public static String selectionAfterRemoval(String removedPath, String chosenPath) {
        if (selected(removedPath, chosenPath)) return "";
        return chosenPath == null ? "" : chosenPath;
    }
}
