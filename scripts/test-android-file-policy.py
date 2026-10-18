#!/usr/bin/env python3
"""检查 Android 文件清理策略是否集中在共享 FilePolicy。"""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FILE_POLICY = ROOT / "platforms/android/java/app/msime/android/FilePolicy.java"
UPDATE_API = ROOT / "platforms/android/java/app/msime/android/account/UpdateApi.java"
PROFILE_PAGE = ROOT / "platforms/android/java/app/msime/android/home/ProfilePage.java"
ABOUT_PAGE = ROOT / "platforms/android/java/app/msime/android/home/AboutPage.java"
DEVELOPER_PAGE = ROOT / "platforms/android/java/app/msime/android/home/DeveloperPage.java"
UPDATE_JOB_SERVICE = ROOT / "platforms/android/java/app/msime/android/home/UpdateJobService.java"


def main() -> None:
    file_policy = FILE_POLICY.read_text(encoding="utf-8")
    update_api = UPDATE_API.read_text(encoding="utf-8")
    profile_page = PROFILE_PAGE.read_text(encoding="utf-8")
    about_page = ABOUT_PAGE.read_text(encoding="utf-8")
    developer_page = DEVELOPER_PAGE.read_text(encoding="utf-8")
    update_job_service = UPDATE_JOB_SERVICE.read_text(encoding="utf-8")
    required = (
        "public static void deleteQuietly(File file)",
        "if (file.exists() && !file.delete()) file.deleteOnExit();",
    )
    missing = [snippet for snippet in required if snippet not in file_policy]
    if missing:
        raise AssertionError("FilePolicy 缺少共享文件清理策略：" + ", ".join(missing))
    if "private static void deleteQuietly(File file)" in update_api:
        raise AssertionError("UpdateApi 仍保留重复的文件清理实现")
    if "FilePolicy.deleteQuietly(partial);" not in update_api:
        raise AssertionError("UpdateApi 没有调用共享文件清理策略")
    if "private static void deleteQuietly(File file)" in profile_page:
        raise AssertionError("ProfilePage 仍保留重复的文件清理实现")
    if "FilePolicy.deleteQuietly(file);" not in profile_page:
        raise AssertionError("ProfilePage 没有调用共享文件清理策略")
    if "if (!apk.delete()) apk.deleteOnExit();" in about_page:
        raise AssertionError("AboutPage 仍保留重复的文件清理实现")
    if "FilePolicy.deleteQuietly(apk);" not in about_page:
        raise AssertionError("AboutPage 没有调用共享文件清理策略")
    if "if (zip.exists() && !zip.delete()) zip.deleteOnExit();" in developer_page:
        raise AssertionError("DeveloperPage 仍保留重复的文件清理实现")
    if "if (!stale.delete()) stale.deleteOnExit();" in developer_page:
        raise AssertionError("DeveloperPage 仍保留重复的文件清理实现")
    # MCP 上传从 5c 起由 Rust 从诊断记录现读现建（#8201 迁移计划），不再先写一份临时 zip；剩下的是导出诊断包时的归档和生成失败时的目标文件。
    if "FilePolicy.deleteQuietly(archive);" not in developer_page \
            or "FilePolicy.deleteQuietly(destination);" not in developer_page:
        raise AssertionError("DeveloperPage 没有完整调用共享文件清理策略")
    if "FilePolicy.deleteQuietly(stale);" not in developer_page:
        raise AssertionError("DeveloperPage 没有清理过期文件")
    if "if (!apk.delete()) apk.deleteOnExit();" in update_job_service:
        raise AssertionError("UpdateJobService 仍保留重复的文件清理实现")
    if "FilePolicy.deleteQuietly(apk);" not in update_job_service:
        raise AssertionError("UpdateJobService 没有调用共享文件清理策略")
    print("android file policy: quiet deletion is shared")


if __name__ == "__main__":
    main()
