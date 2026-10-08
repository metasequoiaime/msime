import android.content.res.Configuration;
import app.msime.android.HardwareKeyboardModePolicy;

public final class HardwareKeyboardModePolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 配置：有键盘且没合上才算接着；没有键盘或未定义都不算。
        check(HardwareKeyboardModePolicy.attached(Configuration.KEYBOARD_QWERTY, Configuration.HARDKEYBOARDHIDDEN_NO),
            "qwerty keyboard in use");
        check(HardwareKeyboardModePolicy.attached(Configuration.KEYBOARD_QWERTY,
            Configuration.HARDKEYBOARDHIDDEN_UNDEFINED), "hidden state unknown still counts");
        check(!HardwareKeyboardModePolicy.attached(Configuration.KEYBOARD_QWERTY,
            Configuration.HARDKEYBOARDHIDDEN_YES), "folded keyboard cover");
        check(!HardwareKeyboardModePolicy.attached(Configuration.KEYBOARD_NOKEYS,
            Configuration.HARDKEYBOARDHIDDEN_NO), "no keyboard");
        check(!HardwareKeyboardModePolicy.attached(Configuration.KEYBOARD_UNDEFINED,
            Configuration.HARDKEYBOARDHIDDEN_NO), "undefined keyboard");

        // 只有真实的字母型键盘设备算实体键盘打字。
        check(HardwareKeyboardModePolicy.physicalTyping(true, false, true), "bluetooth or pogo keyboard");
        check(!HardwareKeyboardModePolicy.physicalTyping(true, true, true), "injected key events");
        check(!HardwareKeyboardModePolicy.physicalTyping(true, false, false), "volume and headset keys");
        check(!HardwareKeyboardModePolicy.physicalTyping(false, false, true), "event without a device");

        // 配置变化：键盘拔掉或合上时退出；旋转之类不动；从未报告过键盘的设备保持打字时进入的状态。
        check(!HardwareKeyboardModePolicy.afterConfiguration(true, true, false), "keyboard detached");
        check(HardwareKeyboardModePolicy.afterConfiguration(true, true, true), "rotation keeps the mode");
        check(HardwareKeyboardModePolicy.afterConfiguration(true, false, false),
            "device that never reports a keyboard keeps the typed-in mode");
        check(!HardwareKeyboardModePolicy.afterConfiguration(false, false, true),
            "attaching alone does not collapse the keyboard");

        // 输入窗口：系统要显示就显示；系统因实体键盘不显示时，走引擎的输入框仍要显示候选条。
        check(HardwareKeyboardModePolicy.showInputView(true, false, false), "system decision kept");
        check(HardwareKeyboardModePolicy.showInputView(false, true, true), "candidate bar for engine editors");
        check(!HardwareKeyboardModePolicy.showInputView(false, true, false), "password field stays hidden");
        check(!HardwareKeyboardModePolicy.showInputView(false, false, true), "no keyboard, system decides");

        // 系统因实体键盘不显示虚拟键盘时，窗口只给候选条；系统要显示时不替用户收起。
        check(HardwareKeyboardModePolicy.afterSystemDecision(false, false, true, false), "system hid the keyboard");
        check(!HardwareKeyboardModePolicy.afterSystemDecision(false, true, true, false),
            "show-virtual-keyboard setting keeps the soft keyboard");
        check(!HardwareKeyboardModePolicy.afterSystemDecision(false, false, false, false), "no keyboard attached");
        check(HardwareKeyboardModePolicy.afterSystemDecision(true, true, false, false), "typed-in mode kept");
        // 点过展开键后，系统再次询问（挪光标、换输入框）不把键盘收回去；只有实体键盘打字才重新收起。
        check(!HardwareKeyboardModePolicy.afterSystemDecision(false, false, true, true),
            "expand key sticks across later show requests");
        check(HardwareKeyboardModePolicy.afterSystemDecision(true, false, true, true),
            "physical typing after an expand collapses again");
        // 展开的选择只在同一次接着期间有效：拔掉、合上或重新接上都清掉。
        check(HardwareKeyboardModePolicy.userExpandedAfterConfiguration(true, true, true),
            "rotation keeps the expand choice");
        check(!HardwareKeyboardModePolicy.userExpandedAfterConfiguration(true, true, false),
            "detaching clears the expand choice");
        check(!HardwareKeyboardModePolicy.userExpandedAfterConfiguration(true, false, true),
            "reattaching starts from the system setting");
        check(!HardwareKeyboardModePolicy.userExpandedAfterConfiguration(false, true, true), "never expanded");

        // 键区收起：面板打开时临时展开。
        check(HardwareKeyboardModePolicy.keysCollapsed(true, false), "collapsed to the candidate bar");
        check(!HardwareKeyboardModePolicy.keysCollapsed(true, true), "panel needs the key area");
        check(!HardwareKeyboardModePolicy.keysCollapsed(false, false), "soft keyboard mode");

        // 序号：1–9，与数字行选词对应。
        check("1 ".equals(HardwareKeyboardModePolicy.candidatePrefix(true, true, false, 0)), "first slot");
        check("9 ".equals(HardwareKeyboardModePolicy.candidatePrefix(true, true, false, 8)), "ninth slot");
        check(HardwareKeyboardModePolicy.candidatePrefix(true, true, false, 9).isEmpty(), "no tenth digit");
        check(HardwareKeyboardModePolicy.candidatePrefix(true, true, false, -1).isEmpty(), "invalid slot");
        check(HardwareKeyboardModePolicy.candidatePrefix(false, true, false, 0).isEmpty(), "touch typing");
        check(HardwareKeyboardModePolicy.candidatePrefix(true, false, false, 0).isEmpty(),
            "number row selection off");
        check(HardwareKeyboardModePolicy.candidatePrefix(true, true, true, 0).isEmpty(), "direct English");
        System.out.println("Android hardware keyboard mode: attachment, physical typing, window, sticky expand, collapse and numbering passed");
    }
}
