#!/usr/bin/env python3
"""Linux 各版本的身份没有漂移，几个版本同时装、同时跑时互不越界。

版本表的 Linux 段（`scripts/test-editions.py` 检查它本身）由 `platforms/linux/scripts/edition_linux.py` 交给三处：提交进仓库的 `platforms/linux/src/core/LinuxEdition.h`、按版本改写的脚本和数据文件，以及 CMake（`platforms/linux/cmake/Edition.cmake`）。这里检查：

- `LinuxEdition.h` 与版本表一致（`edition_linux.py gen --check`）；
- CMake 交给 `msime_edition_source` 改写的每个文件都有改写规则，否则非 full 的版本在配置阶段才失败；
- 每个版本都能改写全部有规则的文件：规则全部命中，改写后不留下 full 的目录名、包名、单元名、命令名、显示名、Fcitx5 插件名或主题名（`edition_linux.py check`）；
- 两个版本的 Fcitx5 插件被同一个 fcitx5 进程加载时，动作名（`MSIME_EDITION_FCITX5_ADDON "-<名字>"` 展开后）两两不撞：fcitx5 的动作名在整个进程里唯一，撞名的那个注册不上，菜单里就少了它；
- 输入上下文属性名（`registerProperty`）同样随插件名、两两不撞，full 仍是 `msimeState`：属性名也在整个进程里唯一，后注册的那个拿不到槽位，第一次 `propertyFor` 就越界、带着整个 fcitx5 崩掉；
- 每个版本装的 Host API 库文件名（`cmake/Edition.cmake` 的 `MSIME_HOST_LIBRARY_STEM`）两两不同，full 仍是 `libmsime_host_api.so`：库没有 SONAME，插件的 DT_NEEDED 就是这个文件名，同名时同一个 fcitx5 进程里后加载的插件会复用先加载的那个版本的库；
- 每个版本登记在它的默认方案所属的语言下：IBus 组件文件的 `<language>`、IBus 宿主注册引擎时的语言（`MSIME_EDITION_IBUS_LANGUAGE`）和 Fcitx5 输入法条目的 `LangCode`。中文的版本是 `zh` / `zh_CN`，日文、越南文、藏文版是 `ja`、`vi`、`bo`；
- 不带中文主词库（`resources.components` 没有 `chinese-main`）的版本不装落定重排模型：`cmake/Edition.cmake` 按版本表算出 `MSIME_EDITION_CHINESE_MAIN`，CMakeLists.txt 据它清空 `MSIME_SETTLED_MODEL`；
- 不提供中文方案（版本表 `features.handwriting`、`features.offline_glosses` 为 false）的版本不装手写模型和非英文离线释义：`cmake/Edition.cmake` 按版本表算出 `MSIME_EDITION_HANDWRITING` 和 `MSIME_EDITION_OFFLINE_GLOSSES`，CMakeLists.txt 据前者清空 `MSIME_HANDWRITING_MODEL_DIR`（模型和它的许可证都不装，打包也不再要求模型），据后者跳过 `MSIME_OFFLINE_GLOSSES`；IBus 和 Fcitx5 的菜单按 `LinuxEdition.h` 的 `MSIME_EDITION_HANDWRITING` 不列手写识别板；
- C++ 源码不再自己写 full 的每用户目录、IBus 引擎名和设置命令，CMake 不再自己写 `msime-client` 子目录，否则那一处在其他版本里仍指向 full。
"""

from __future__ import annotations

import importlib.util
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LINUX = ROOT / "platforms/linux"
GENERATOR = LINUX / "scripts/edition_linux.py"
FCITX_ENGINE = LINUX / "fcitx5/FcitxEngine.cpp"
IBUS_MAIN = LINUX / "src/entrypoints/ibus_main.cpp"
# 默认方案到它所属语言的（IBus 语言，Fcitx5 LangCode），与生成器分开写一份，生成器的表写错时这里能看出来。
EXPECTED_LANGUAGES = {
    "quanpin": ("zh", "zh_CN"),
    "shuangpin": ("zh", "zh_CN"),
    "wubi": ("zh", "zh_CN"),
    "japanese": ("ja", "ja"),
    "vietnamese": ("vi", "vi"),
    "tibetan": ("bo", "bo"),
}
# C++ 里不能再出现的 full 的字面量：每用户目录、IBus 引擎名、设置命令和主题名都要从 LinuxEdition.h 取。
CPP_FORBIDDEN = [
    (re.compile(r'"msime-client[/"]'), "the per-user directory"),
    (re.compile(r'"/msime-client/'), "the per-user directory"),
    (re.compile(r'"msime-linux"'), "the IBus engine name"),
    (re.compile(r'"msime-linux-(setup|settings)"'), "a user command"),
    (re.compile(r'registerAction\("msime-'), "a Fcitx5 action name"),
    (re.compile(r'registerProperty\("'), "the Fcitx5 input context property name"),
    (re.compile(r'kFcitxCandidateTheme = "msime"'), "the Fcitx5 theme name"),
]
CMAKE_FILES = [LINUX / "CMakeLists.txt", LINUX / "fcitx5/CMakeLists.txt", LINUX / "cmake/packaging.cmake"]


def load_generator():
    spec = importlib.util.spec_from_file_location("edition_linux", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def code_lines(path: pathlib.Path):
    """源码里不是整行注释的行。"""
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.lstrip()
        if not stripped.startswith(("//", "#")) or stripped.startswith(("#define", "#include")):
            yield number, line


def main() -> int:
    errors: list[str] = []
    for command in (["gen", "--check"], ["check"]):
        result = subprocess.run([sys.executable, str(GENERATOR), *command], capture_output=True, text=True)
        if result.returncode != 0:
            errors.append(f"edition_linux.py {' '.join(command)}: {(result.stdout + result.stderr).strip()}")

    generator = load_generator()
    editions = generator.linux_editions()
    suffixes = set(re.findall(r'MSIME_EDITION_FCITX5_ADDON "-([a-z0-9-]+)"', FCITX_ENGINE.read_text(encoding="utf-8")))
    if not suffixes:
        errors.append(f"{FCITX_ENGINE.relative_to(ROOT)} names no action through MSIME_EDITION_FCITX5_ADDON; update this check")
    seen: dict[str, str] = {}
    for entry in editions:
        addon = entry["platforms"]["linux"]["fcitx5_addon"]
        for suffix in sorted(suffixes):
            name = f"{addon}-{suffix}"
            if name in seen:
                errors.append(f"Fcitx5 action {name!r} of edition {entry['id']} is also edition {seen[name]}'s; two editions loaded into one fcitx5 would collide")
            seen[name] = entry["id"]

    engine_text = FCITX_ENGINE.read_text(encoding="utf-8")
    properties = re.findall(r'registerProperty\(MSIME_EDITION_FCITX5_ADDON "([A-Za-z]+)"', engine_text)
    if len(properties) != engine_text.count("registerProperty("):
        errors.append(f"{FCITX_ENGINE.relative_to(ROOT)} registers an input context property not named through MSIME_EDITION_FCITX5_ADDON; update this check")
    owners: dict[str, str] = {}
    for entry in editions:
        addon = entry["platforms"]["linux"]["fcitx5_addon"]
        for suffix in properties:
            name = f"{addon}{suffix}"
            if entry["id"] == "full" and name != "msimeState":
                errors.append(f"full's Fcitx5 input context property is {name!r}, not msimeState")
            if name in owners:
                errors.append(f"Fcitx5 input context property {name!r} of edition {entry['id']} is also edition {owners[name]}'s")
            owners[name] = entry["id"]

    # 宿主库文件名：Edition.cmake 用 full 的名字，其他版本在后面加 _<id>。这里按同样的规则展开，并确认 Edition.cmake 仍是这条规则。
    edition_cmake = (LINUX / "cmake/Edition.cmake").read_text(encoding="utf-8")
    if 'set(MSIME_HOST_LIBRARY_STEM "msime_host_api")' not in edition_cmake or 'set(MSIME_HOST_LIBRARY_STEM "msime_host_api_${MSIME_EDITION}")' not in edition_cmake:
        errors.append("cmake/Edition.cmake no longer names the host library msime_host_api (full) / msime_host_api_<id>; update this check")
    libraries: dict[str, str] = {}
    for entry in editions:
        library = "libmsime_host_api.so" if entry["id"] == "full" else f"libmsime_host_api_{entry['id']}.so"
        if library in libraries:
            errors.append(f"host library {library} of edition {entry['id']} is also edition {libraries[library]}'s")
        libraries[library] = entry["id"]

    # 登记语言：按每个版本改写 IBus 组件文件和 Fcitx5 输入法条目（full 原样），再从生成的头文件里取出该版本的 MSIME_EDITION_IBUS_LANGUAGE。
    ibus_source = (LINUX / "data/msime-linux.xml.in").read_text(encoding="utf-8")
    fcitx_source = (LINUX / "fcitx5/msime-inputmethod.conf").read_text(encoding="utf-8")
    header = (LINUX / "src/core/LinuxEdition.h").read_text(encoding="utf-8")
    for entry in editions:
        expected = EXPECTED_LANGUAGES.get(entry["default_scheme"])
        if expected is None:
            errors.append(f"edition {entry['id']}: no Linux language is known for its default scheme {entry['default_scheme']!r}; add it to EXPECTED_LANGUAGES")
            continue
        try:
            names = generator.identity(entry)
            ibus = generator.render_text("data/msime-linux.xml.in", ibus_source, names)
            fcitx = generator.render_text("fcitx5/msime-inputmethod.conf", fcitx_source, names)
        except SystemExit as error:
            errors.append(f"edition {entry['id']}: {error}")
            continue
        ibus_languages = re.findall(r"<language>([^<]*)</language>", ibus)
        fcitx_languages = re.findall(r"^LangCode=(.*)$", fcitx, re.MULTILINE)
        block = re.search(rf"defined\(MSIME_EDITION_{entry['id'].upper()}\)\n(.*?)(?=^#(?:elif|endif))", header, re.MULTILINE | re.DOTALL)
        header_language = re.search(r'^#define MSIME_EDITION_IBUS_LANGUAGE "([^"]*)"$', block.group(1), re.MULTILINE) if block else None
        if ibus_languages != [expected[0]]:
            errors.append(f"edition {entry['id']}: the IBus component declares languages {ibus_languages}, but its default scheme {entry['default_scheme']} belongs under {expected[0]!r}")
        if fcitx_languages != [expected[1]]:
            errors.append(f"edition {entry['id']}: the Fcitx5 input method entry declares LangCode {fcitx_languages}, but its default scheme {entry['default_scheme']} belongs under {expected[1]!r}")
        if not header_language or header_language.group(1) != expected[0]:
            errors.append(f"edition {entry['id']}: LinuxEdition.h does not define MSIME_EDITION_IBUS_LANGUAGE as {expected[0]!r}")
    if "MSIME_EDITION_IBUS_LANGUAGE" not in IBUS_MAIN.read_text(encoding="utf-8"):
        errors.append(f"{IBUS_MAIN.relative_to(ROOT)} no longer registers the IBus engine under MSIME_EDITION_IBUS_LANGUAGE; every edition would be listed under one language")

    # 落定重排模型只装给带中文主词库的版本。
    if "set(MSIME_EDITION_CHINESE_MAIN ON)" not in edition_cmake or not re.search(r'if\(NOT MSIME_EDITION_CHINESE_MAIN\)\n(?:  #.*\n|  if\(MSIME_SETTLED_MODEL\)\n.*\n  endif\(\)\n)*  set\(MSIME_SETTLED_MODEL ""\)', (LINUX / "CMakeLists.txt").read_text(encoding="utf-8")):
        errors.append("cmake/Edition.cmake and CMakeLists.txt no longer keep the settled model out of editions without chinese-main; update this check")

    # 手写模型和非英文离线释义只装给提供中文方案的版本，菜单里的手写识别板也只给它们。
    linux_cmake = (LINUX / "CMakeLists.txt").read_text(encoding="utf-8")
    if "foreach(msime_edition_feature handwriting offline_glosses)" not in edition_cmake:
        errors.append("cmake/Edition.cmake no longer derives MSIME_EDITION_HANDWRITING and MSIME_EDITION_OFFLINE_GLOSSES from the edition table; update this check")
    if not re.search(r'if\(NOT MSIME_EDITION_HANDWRITING\)\n(?:  .*\n)*?  set\(MSIME_HANDWRITING_MODEL_DIR ""\)', linux_cmake) or "elseif(MSIME_ENABLE_PACKAGING AND MSIME_EDITION_HANDWRITING)" not in linux_cmake:
        errors.append("CMakeLists.txt no longer keeps the handwriting model out of editions without handwriting; update this check")
    if "if(MSIME_OFFLINE_GLOSSES AND NOT MSIME_EDITION_OFFLINE_GLOSSES)" not in linux_cmake:
        errors.append("CMakeLists.txt no longer keeps the offline glosses out of editions without a Chinese scheme; update this check")
    header = (LINUX / "src/core/LinuxEdition.h").read_text(encoding="utf-8")
    for entry in editions:
        block = re.search(rf"defined\(MSIME_EDITION_{entry['id'].upper()}\)\n(.*?)(?:#elif|#endif)", header, re.S)
        expected_flag = "1" if entry["features"]["handwriting"] else "0"
        if not block or f"#define MSIME_EDITION_HANDWRITING {expected_flag}" not in block.group(1):
            errors.append(f"edition {entry['id']}: LinuxEdition.h does not define MSIME_EDITION_HANDWRITING as {expected_flag}")
    if "desktop_panel_offered(action)" not in (LINUX / "src/core/ClientEngine.cpp").read_text(encoding="utf-8"):
        errors.append("src/core/ClientEngine.cpp no longer filters the IBus desktop panels by MSIME_EDITION_HANDWRITING; update this check")
    if "if (MSIME_EDITION_HANDWRITING != 0) desktop_tools_menu_.addAction(&handwriting_action_);" not in FCITX_ENGINE.read_text(encoding="utf-8"):
        errors.append("fcitx5/FcitxEngine.cpp no longer leaves handwriting out of the Fcitx5 menu of editions without it; update this check")

    # CMake 里每个 msime_edition_source 的文件都要有规则。路径里的 ${变量} 来自同一个文件的 foreach：脚本按 foreach 列出的名字展开，其余按通配展开到实际存在的文件。
    rules = set(generator.RULES)
    for path in [LINUX / "CMakeLists.txt", LINUX / "fcitx5/CMakeLists.txt"]:
        text = path.read_text(encoding="utf-8")
        for relative in re.findall(r"msime_edition_source\(\w+ ([^)\s]+)\)", text):
            if "${MSIME_EDITION_SCRIPT}" in relative:
                names = re.search(r"foreach\(MSIME_EDITION_SCRIPT ([^)]*)\)", text).group(1).split()
                files = [relative.replace("${MSIME_EDITION_SCRIPT}", name) for name in names]
            else:
                pattern = re.sub(r"\$\{\w+\}", "*", relative)
                files = [found.relative_to(LINUX).as_posix() for found in sorted(LINUX.glob(pattern))]
            if not files:
                errors.append(f"{path.relative_to(ROOT)}: msime_edition_source({relative}) names no file")
            for file in files:
                if file not in rules:
                    errors.append(f"{path.relative_to(ROOT)}: {file} is rendered per edition but has no rules in {GENERATOR.relative_to(ROOT)}")

    sources = sorted((LINUX / "src").rglob("*.cpp")) + sorted((LINUX / "src").rglob("*.h")) + [FCITX_ENGINE]
    for path in sources:
        if path.name == "LinuxEdition.h":
            continue
        for number, line in code_lines(path):
            for pattern, what in CPP_FORBIDDEN:
                if pattern.search(line):
                    errors.append(f"{path.relative_to(ROOT)}:{number}: spells {what} of full; take it from LinuxEdition.h")
    for path in CMAKE_FILES:
        for number, line in code_lines(path):
            if re.search(r"/msime-client[/\"]", line):
                errors.append(f"{path.relative_to(ROOT)}:{number}: spells the msime-client directory; use ${{MSIME_CLIENT_DIRECTORY}}")

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"linux editions: LinuxEdition.h is current, {len(editions)} editions render every script and data file, {len(suffixes)} Fcitx5 actions, the input context property and the host library stay distinct across editions, each edition registers under its default scheme's language, and no host source spells the full identifiers itself")
    return 0


if __name__ == "__main__":
    sys.exit(main())
