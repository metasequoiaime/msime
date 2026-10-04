#pragma once

// 由 platforms/linux/scripts/edition_linux.py 从 shared/contracts/editions.json 生成，不要手改；改了版本表之后运行 `python3 platforms/linux/scripts/edition_linux.py gen` 并提交结果。
//
// 每个 Linux 构建必须定义且只定义一个版本选择宏（MSIME_EDITION_FULL, MSIME_EDITION_PINYIN, MSIME_EDITION_WUBI, MSIME_EDITION_JAPANESE, MSIME_EDITION_VIETNAMESE, MSIME_EDITION_TIBETAN），CMake 按缓存变量 MSIME_EDITION 定义。少了它就停在这里，而不是悄悄编成 full、去读写 full 的状态目录和 socket。
//
// full 的名字与引入版本之前相同；其他版本的每用户目录、IBus 引擎、Fcitx5 插件与动作名、图标和命令名都带版本，所以几个版本可以同时安装，两个版本的 Fcitx5 插件也可以被同一个 fcitx5 进程同时加载。

#if (defined(MSIME_EDITION_FULL) + defined(MSIME_EDITION_PINYIN) + defined(MSIME_EDITION_WUBI) + defined(MSIME_EDITION_JAPANESE) + defined(MSIME_EDITION_VIETNAMESE) + defined(MSIME_EDITION_TIBETAN)) != 1
#error "Define exactly one of MSIME_EDITION_FULL, MSIME_EDITION_PINYIN, MSIME_EDITION_WUBI, MSIME_EDITION_JAPANESE, MSIME_EDITION_VIETNAMESE, MSIME_EDITION_TIBETAN; platforms/linux/cmake/Edition.cmake does this from MSIME_EDITION"
#endif
#if defined(MSIME_EDITION_FULL)
#define MSIME_EDITION_ID "full"
#define MSIME_EDITION_IS_FULL 1
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\350\276\223\345\205\245\346\263\225"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME"
#define MSIME_EDITION_PACKAGE "msime-linux"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\350\276\223\345\205\245\346\263\225"
#define MSIME_EDITION_IBUS_LANGUAGE "zh"
#define MSIME_EDITION_FCITX5_ADDON "msime"
#define MSIME_EDITION_ICON "msime-linux"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime"
#define MSIME_EDITION_DEFAULT_SCHEME "quanpin"
#define MSIME_EDITION_INPUT_SCHEMES "quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"
#define MSIME_EDITION_TEMPORARY_JAPANESE 1
#define MSIME_EDITION_HANDWRITING 1
#elif defined(MSIME_EDITION_PINYIN)
#define MSIME_EDITION_ID "pinyin"
#define MSIME_EDITION_IS_FULL 0
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\346\213\274\351\237\263"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME Pinyin"
#define MSIME_EDITION_PACKAGE "msime-linux-pinyin"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client-pinyin"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux-pinyin"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\346\213\274\351\237\263"
#define MSIME_EDITION_IBUS_LANGUAGE "zh"
#define MSIME_EDITION_FCITX5_ADDON "msime-pinyin"
#define MSIME_EDITION_ICON "msime-linux-pinyin"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-pinyin-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-pinyin-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux.pinyin"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime-pinyin"
#define MSIME_EDITION_DEFAULT_SCHEME "quanpin"
#define MSIME_EDITION_INPUT_SCHEMES "quanpin", "shuangpin"
#define MSIME_EDITION_TEMPORARY_JAPANESE 1
#define MSIME_EDITION_HANDWRITING 1
#elif defined(MSIME_EDITION_WUBI)
#define MSIME_EDITION_ID "wubi"
#define MSIME_EDITION_IS_FULL 0
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\344\272\224\347\254\224"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME Wubi"
#define MSIME_EDITION_PACKAGE "msime-linux-wubi"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client-wubi"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux-wubi"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\344\272\224\347\254\224"
#define MSIME_EDITION_IBUS_LANGUAGE "zh"
#define MSIME_EDITION_FCITX5_ADDON "msime-wubi"
#define MSIME_EDITION_ICON "msime-linux-wubi"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-wubi-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-wubi-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux.wubi"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime-wubi"
#define MSIME_EDITION_DEFAULT_SCHEME "wubi"
#define MSIME_EDITION_INPUT_SCHEMES "wubi"
#define MSIME_EDITION_TEMPORARY_JAPANESE 0
#define MSIME_EDITION_HANDWRITING 1
#elif defined(MSIME_EDITION_JAPANESE)
#define MSIME_EDITION_ID "japanese"
#define MSIME_EDITION_IS_FULL 0
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\346\227\245\350\257\255"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME Japanese"
#define MSIME_EDITION_PACKAGE "msime-linux-japanese"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client-japanese"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux-japanese"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\346\227\245\350\257\255"
#define MSIME_EDITION_IBUS_LANGUAGE "ja"
#define MSIME_EDITION_FCITX5_ADDON "msime-japanese"
#define MSIME_EDITION_ICON "msime-linux-japanese"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-japanese-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-japanese-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux.japanese"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime-japanese"
#define MSIME_EDITION_DEFAULT_SCHEME "japanese"
#define MSIME_EDITION_INPUT_SCHEMES "japanese"
#define MSIME_EDITION_TEMPORARY_JAPANESE 0
#define MSIME_EDITION_HANDWRITING 0
#elif defined(MSIME_EDITION_VIETNAMESE)
#define MSIME_EDITION_ID "vietnamese"
#define MSIME_EDITION_IS_FULL 0
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\350\266\212\345\215\227\350\257\255"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME Vietnamese"
#define MSIME_EDITION_PACKAGE "msime-linux-vietnamese"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client-vietnamese"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux-vietnamese"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\350\266\212\345\215\227\350\257\255"
#define MSIME_EDITION_IBUS_LANGUAGE "vi"
#define MSIME_EDITION_FCITX5_ADDON "msime-vietnamese"
#define MSIME_EDITION_ICON "msime-linux-vietnamese"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-vietnamese-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-vietnamese-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux.vietnamese"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime-vietnamese"
#define MSIME_EDITION_DEFAULT_SCHEME "vietnamese"
#define MSIME_EDITION_INPUT_SCHEMES "vietnamese"
#define MSIME_EDITION_TEMPORARY_JAPANESE 0
#define MSIME_EDITION_HANDWRITING 0
#elif defined(MSIME_EDITION_TIBETAN)
#define MSIME_EDITION_ID "tibetan"
#define MSIME_EDITION_IS_FULL 0
#define MSIME_EDITION_DISPLAY_NAME "\346\260\264\346\235\211\350\227\217\346\226\207"
#define MSIME_EDITION_DISPLAY_NAME_EN "MSIME Tibetan"
#define MSIME_EDITION_PACKAGE "msime-linux-tibetan"
#define MSIME_EDITION_CLIENT_DIRECTORY "msime-client-tibetan"
#define MSIME_EDITION_IBUS_ENGINE "msime-linux-tibetan"
#define MSIME_EDITION_IBUS_LONGNAME "Metasequoia \346\260\264\346\235\211\350\227\217\346\226\207"
#define MSIME_EDITION_IBUS_LANGUAGE "bo"
#define MSIME_EDITION_FCITX5_ADDON "msime-tibetan"
#define MSIME_EDITION_ICON "msime-linux-tibetan"
#define MSIME_EDITION_SETUP_PROGRAM "msime-linux-tibetan-setup"
#define MSIME_EDITION_SETTINGS_PROGRAM "msime-linux-tibetan-settings"
#define MSIME_EDITION_TAURI_IDENTIFIER "app.msime.linux.tibetan"
#define MSIME_EDITION_TELEMETRY_DIRECTORY "msime-tibetan"
#define MSIME_EDITION_DEFAULT_SCHEME "tibetan"
#define MSIME_EDITION_INPUT_SCHEMES "tibetan"
#define MSIME_EDITION_TEMPORARY_JAPANESE 0
#define MSIME_EDITION_HANDWRITING 0
#endif
