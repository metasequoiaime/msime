#!/usr/bin/env python3
"""编译执行真实宿主提示方法，验证粤语双拼及会话空窗的键面。"""

from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"

HARNESS = r'''
package app.msime.android;
import java.util.*;

public final class HostHintsSmoke {
    static final class View { int scheme; String profile;
        View(int scheme, String profile) { this.scheme=scheme; this.profile=profile; } }
    static final class InputViewValuePolicy {
        static int scheme(View view, int fallback) { return view.scheme; }
        static String textOr(View view, String key, String fallback) {
            return key.equals("shuangpin_profile") ? view.profile : "none";
        }
    }
    static final class Selected {
        String id="cantonese_shuangpin";
        String preferenceId() { return id; }
    }
    static final class ShuangpinHintButton {
        String hint="";
        void setHintText(String value) { hint=value; }
        void setHintColor(int color) { }
        void setContentDescription(String value) { }
    }
    static final class Color { static int parseColor(String value) { return 0; } }
    static final class Skin { String accent() { return "#123456"; } }
    static final class LetterCase { boolean usesUppercase() { return false; } }
    static final class LetterKeyFacePolicy {
        static String accessibilityLabel(String key, boolean chinese, boolean local, boolean shifted) { return key; }
    }
    static final class NativeClient {
        static String requested="";
        static String shuangpinKeyHints(String profile) {
            requested=profile;
            return "{\"ok\":true,\"value\":{\"I\":\"gw / i\"}}";
        }
    }
    View view=new View(5, "");
    Selected selectedScheme=new Selected();
    long session=1;
    boolean dedicatedEnglish=false, shuangpinKeyHintsEnabled=true;
    String remembered="", shuangpinHintsProfile="";
    Map<String,String> shuangpinHints=Map.of();
    Skin skin=new Skin();
    LetterCase letterCase=new LetterCase();
    List<ShuangpinHintButton> shuangpinKeyButtons=List.of(new ShuangpinHintButton());
    List<String> shuangpinKeyInputs=List.of("i");
    void rememberShuangpinProfile(String value) { remembered=value; }
    String shuangpinProfileHint() { return remembered; }
    boolean engineStartupPending() { return false; }
    boolean letterCaseSchemeActive() { return false; }

    __METHOD__

    static void check(boolean ok, String message) {
        if (!ok) throw new AssertionError(message);
    }
    public static void main(String[] args) {
        HostHintsSmoke host=new HostHintsSmoke();
        host.updateShuangpinKeyHints();
        check(host.shuangpinKeyButtons.get(0).hint.equals("gw / i"), "粤语双拼必须显示引擎键面提示");
        check(NativeClient.requested.equals("cantonese"), "粤语双拼必须请求保留键面");
        host.view=null;
        host.updateShuangpinKeyHints();
        check(host.shuangpinKeyButtons.get(0).hint.equals("gw / i"), "会话空窗仍沿用粤语双拼键面");
        host.selectedScheme.id="cantonese";
        host.updateShuangpinKeyHints();
        check(host.shuangpinKeyButtons.get(0).hint.isEmpty(), "切到完整粤拼后不能沿用粤语双拼提示");
        host.view=new View(5, "");
        host.updateShuangpinKeyHints();
        check(host.shuangpinKeyButtons.get(0).hint.isEmpty(), "普通粤拼不显示双拼提示");
        host.view=new View(1, "xiaohe");
        host.updateShuangpinKeyHints();
        check(!host.shuangpinKeyButtons.get(0).hint.isEmpty(), "普通话双拼继续显示提示");
        host.shuangpinKeyHintsEnabled=false;
        host.updateShuangpinKeyHints();
        check(host.shuangpinKeyButtons.get(0).hint.isEmpty(), "关闭提示开关继续生效");
        System.out.println("Android 宿主双拼键面：6 组行为通过");
    }
}
'''


def main():
    if not shutil.which("javac") or not shutil.which("java"):
        print("skipped: Android 宿主键面行为需要 JDK")
        return
    source = (JAVA / "core/MSIMEInputService.java").read_text()
    start = source.index("    private void updateShuangpinKeyHints() {")
    end = source.index("\n    private void updateAutomaticCapitalization()", start)
    method = source[start:end]
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        harness = root / "HostHintsSmoke.java"
        harness.write_text(HARNESS.replace("__METHOD__", method))
        subprocess.run([
            "javac", "-encoding", "UTF-8", "-d", directory,
            str(JAVA / "keyboard/ShuangpinKeyHintPolicy.java"),
            str(JAVA / "policy/InputSchemeTraits.java"),
            str(JAVA / "TextPolicy.java"), str(harness),
        ], check=True)
        subprocess.run(["java", "-cp", directory, "app.msime.android.HostHintsSmoke"], check=True)


if __name__ == "__main__":
    main()
