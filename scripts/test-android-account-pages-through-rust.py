#!/usr/bin/env python3
"""Android 设置页面改调 Rust 以后（#8201 迁移计划 5c），页面一侧要做的那几件事还接着。

这些页面在 Gradle 里才编得到（它们引用 AndroidX），check-host 的 JVM 冒烟碰不到，Rust 那一半由 client-services 和 host-api 的测试覆盖。这里逐条盯住页面的接线：退出登录撤销服务端会话、退出时取消进行中的流式对话和 AI 皮肤、「获取」皮肤先下载再存库、AI 皮肤只在 Rust 生成那一步不带 Retry-After 的 account_unavailable 时藏入口（注册匿名账号失败不算）、云端数据的语音样本、反馈截图在 Rust 的限制以内、语音设置里能删语音样本。
"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"


def read(relative: str) -> str:
    return (JAVA / relative).read_text(encoding="utf-8")


def method_body(source: str, signature: str) -> str:
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening : index + 1]
    raise AssertionError(f"方法未闭合: {signature}")


class AndroidAccountPagesThroughRust(unittest.TestCase):
    def test_sign_out_button_revokes_the_server_session(self):
        # 计划第 10 节：「退出登录」撤销服务端会话（logout{all:false}），断网时本机照样退出。
        profile = read("home/ProfilePage.java")
        self.assertIn("SignIn.logout(context, expectedSessionId)", method_body(profile, "private void confirmSignOut()"))
        logout = method_body(read("account/DeviceDataApi.java"), "public void logout()")
        self.assertIn('op("logout").put("all", false)', logout)
        sign_in = method_body(read("home/SignIn.java"), "static Logout logout(Context context, String expectedUserId)")
        self.assertIn("signOut(context);", sign_in)
        self.assertIn("AccountFailureText.signedOutLocally(code)", sign_in)

    def test_sign_out_cancels_streams_in_flight(self):
        # Rust 只在回答完整以后核对账号：退出登录时由 Java 取消进行中的对话和 AI 皮肤。
        sign_out = method_body(read("home/SignIn.java"), "static void signOut(Context context)")
        self.assertIn("RustAccount.cancelForSignOut();", sign_out)
        rust = read("account/RustAccount.java")
        self.assertIn("CHATS.cancelAll();", rust)
        self.assertIn("SKINS.cancelAll();", rust)

    def test_getting_a_skin_downloads_before_saving(self):
        # 决定 14：keyboard_skin_download 加皮肤库；下载在前，断网时不存。
        get = method_body(read("home/CommunityFragment.java"), "private void get(CommunityCatalog.Item item)")
        download = get.index(".download(item)")
        install = get.index("CommunityCatalog.install(")
        self.assertLess(download, install)
        self.assertIn("if (download.failed()) return download.failure();", get)
        self.assertNotIn("recordDownload", read("community/CommunityCatalog.java"))

    def test_ai_skin_hides_only_without_retry_after(self):
        page = read("home/AiSkinPage.java")
        self.assertIn("AccountFailureText.aiSkinUnavailable(failure.code, failure.retryAfterSeconds)", page)
        policy = method_body(read("account/AccountFailureText.java"), "public static boolean aiSkinUnavailable(")
        self.assertIn('"account_unavailable".equals(code) && retryAfterSeconds <= 0', policy)

    def test_ai_skin_anonymous_registration_failure_does_not_hide(self):
        # ensure_anonymous 断网时也是不带 Retry-After 的 account_unavailable：生成前注册匿名账号失败要换成自己的码，不能让页面把入口藏一天。
        generate = method_body(read("account/RustAccount.java"), "public static JSONArray aiSkinGenerate(")
        retry = generate.index('"account_unauthorized".equals(failure.code)')
        register = generate.index("ensureAnonymous(context);", retry)
        self.assertIn("AccountFailureText.aiSkinAnonymousFailure(failure.code)", generate[register:])

    def test_cloud_data_shows_voice_samples(self):
        profile = read("home/ProfilePage.java")
        self.assertIn("AccountFailureText.anonymousVoice(state.data().anonymousVoice())", profile)
        delete = method_body(profile, "private void confirmDeleteData()")
        self.assertIn("DeviceDataApi.DELETABLE_SECTIONS", delete)
        self.assertIn("AccountFailureText.voiceSamplesLeft(", delete)
        account = method_body(profile, "private void confirmDeleteAccount()")
        self.assertIn("AccountFailureText.voiceSamplesLeft(", account)
        self.assertRegex(read("account/DeviceDataApi.java"),
                         r'DELETABLE_SECTIONS =\s*List\.of\("preferences", "dictionary", "phrases", "clipboard", "voice"\)')

    def test_voice_settings_delete_voice_samples(self):
        voice = read("home/VoicePage.java")
        self.assertIn("this::confirmDeleteVoiceSamples", voice)
        self.assertIn("DeviceDataApi.deleteVoiceSamples(context);", method_body(voice, "private void confirmDeleteVoiceSamples()"))
        self.assertIn('op("voice_delete")', read("account/DeviceDataApi.java"))

    def test_feedback_screenshots_fit_rust(self):
        # 差异 25：Rust 不重新编码截图，超限是 account_too_large；页面重新编码的每一张都核对一遍。
        page = method_body(read("home/FeedbackPage.java"), "@Nullable private static byte[] reencode(")
        self.assertIn("FeedbackApi.fitsRust(jpeg.length, bitmap.getWidth(), bitmap.getHeight())", page)

    def test_pages_do_not_read_rust_on_the_main_thread(self):
        # M81：读登录状态要等 Rust 的会话锁，页面的回调里不能再调 sessionId()。
        for relative in ("home/ChatFragment.java", "home/DownloadPage.java", "home/AiSkinPage.java",
                         "home/CommunityFragment.java", "home/AccountFragment.java"):
            with self.subTest(page=relative):
                self.assertNotRegex(read(relative), r"new BackendAccount\([^)]*\)\.sessionId\(\)")


if __name__ == "__main__":
    unittest.main()
