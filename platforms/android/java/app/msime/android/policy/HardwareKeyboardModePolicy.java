package app.msime.android;

import android.content.res.Configuration;

/**
 * 外接硬件键盘时的「候选条模式」（#5584）：用实体键盘打字时软键盘的键区收起，只留顶部一行（空闲时工具栏，组词时候选条），候选前面标上 1–9 的序号，与数字行选词对应。
 *
 * <p>进入这个模式的信号是「真的在用实体键盘打字」：一次来自非虚拟、字母型键盘设备的按下。只看 `Configuration.keyboard` 会误伤：有的平板常驻报告 QWERTY，系统「使用实体键盘时显示虚拟键盘」打开的用户也明确要软键盘。离开的信号是用户点候选条上的展开键，或配置报告键盘拔掉 / 合上（从有到无）；点过展开键后，同一次接着期间系统的「不显示虚拟键盘」不再把键盘收回去，只有下一次实体键盘打字才收起。这里只做判断，不依赖 Android 运行时，供 JVM 冒烟直接调用。
 */
public final class HardwareKeyboardModePolicy {
    private HardwareKeyboardModePolicy() { }

    /** 配置是否报告一块可用的实体键盘：有键盘且没被合上（翻盖、键盘盖翻到背面时 `hardKeyboardHidden` 为 YES）。 */
    public static boolean attached(int keyboard, int hardKeyboardHidden) {
        return keyboard != Configuration.KEYBOARD_NOKEYS && keyboard != Configuration.KEYBOARD_UNDEFINED
            && hardKeyboardHidden != Configuration.HARDKEYBOARDHIDDEN_YES;
    }

    /**
     * 这次按键是否来自实体键盘：有设备、不是虚拟设备（`adb shell input`、无障碍注入、导航栏返回键都是虚拟设备），且是字母型键盘（音量键、耳机按键所在的设备不是）。
     */
    public static boolean physicalTyping(boolean hasDevice, boolean virtualDevice, boolean alphabetic) {
        return hasDevice && !virtualDevice && alphabetic;
    }

    /**
     * 配置变化后模式是否保留：键盘从「有」变成「无」时退出；其余情况（旋转、窗口变化、配置一直报告没有键盘的设备）保持原状。
     */
    public static boolean afterConfiguration(boolean active, boolean wasAttached, boolean nowAttached) {
        if (wasAttached && !nowAttached) return false;
        return active;
    }

    /**
     * 是否允许显示输入窗口。系统在接着实体键盘、且「显示虚拟键盘」关着时不显示输入法窗口，这时组词的候选无处可看；用引擎的输入框在接着实体键盘时也显示（窗口里只有候选条）。密码、数字这类不走引擎的输入框照系统的意思。
     */
    public static boolean showInputView(boolean systemShows, boolean attached, boolean engineEditor) {
        return systemShows || (attached && engineEditor);
    }

    /**
     * 系统判断之后模式是否打开：接着实体键盘、系统按「使用实体键盘时显示虚拟键盘」关着而不显示输入法窗口时，用户已经表明不要虚拟键盘，窗口若因 {@link #showInputView} 仍显示，就只给候选条；其余情况保持原状。用户在这块键盘接着期间点过展开键（`userExpanded`）时不再替他收起：系统每次 `showSoftInput`（点输入框挪光标、换输入框）都会重新问一遍，照系统开关收起会让展开键只管到下一次点击。
     */
    public static boolean afterSystemDecision(boolean active, boolean systemShows, boolean attached,
            boolean userExpanded) {
        return active || (!systemShows && attached && !userExpanded);
    }

    /**
     * 配置变化后「用户点过展开键」是否还算数：只在同一次接着期间有效，键盘拔掉、合上或重新接上（报告从有到无或从无到有）时清掉，下次接上照系统开关重新判断。
     */
    public static boolean userExpandedAfterConfiguration(boolean userExpanded, boolean wasAttached,
            boolean nowAttached) {
        return userExpanded && wasAttached == nowAttached;
    }

    /**
     * 键区此刻是否收起：模式开着，且没有盖在键区上的面板（工具栏面板、展开候选、内联高度条、手写）。面板要用键区的高度，打开时整副键盘临时展开，关上后回到候选条。
     */
    public static boolean keysCollapsed(boolean active, boolean overlayOpen) {
        return active && !overlayOpen;
    }

    /**
     * 候选前面的序号：模式开着、共享 `number_row_selection` 开着、不是英文直输，且槽位在前十个以内时为「N 」，否则为空。数字行选的就是这一页第 N 个，序号和按键一一对应，所以第十个标「0」。
     */
    public static String candidatePrefix(boolean active, boolean numberRowSelection, boolean dedicatedEnglish,
            int slot) {
        if (!active || !numberRowSelection || dedicatedEnglish || slot < 0 || slot >= 10) return "";
        return NumberRowSelectionPolicy.digitOfSlot(slot) + " ";
    }
}
