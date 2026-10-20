#!/usr/bin/env python3
"""构建独立测试包，暂时注入合成资源与原生编辑器，始终恢复产品源文件。"""

import contextlib
import json
import pathlib
import shutil
import struct
import subprocess
import zlib


HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[3]
HARMONY = ROOT / 'platforms/harmony'
SOURCE = HARMONY / 'entry/src/main'
OUTPUT = ROOT / 'target/harmony-tests/device'
SIGNATURE = b'\x89PNG\r\n\x1a\n'


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


def apng(width, height, colors):
    result = SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
    result += chunk(b'acTL', struct.pack('>II', len(colors), 0))
    sequence = 0
    for index, color in enumerate(colors):
        result += chunk(b'fcTL', struct.pack('>IIIIIHHBB', sequence, width, height, 0, 0, 1, 2, 0, 0))
        sequence += 1
        raw = b''.join(b'\x00' + bytes(color) * width for _ in range(height))
        compressed = zlib.compress(raw)
        if index == 0:
            result += chunk(b'IDAT', compressed)
        else:
            result += chunk(b'fdAT', struct.pack('>I', sequence) + compressed)
            sequence += 1
    return result + chunk(b'IEND', b'')


@contextlib.contextmanager
def temporary_sources(replacements, additions):
    originals = {path: path.read_bytes() for path in replacements}
    if any(path.exists() for path in additions):
        raise RuntimeError('测试注入文件已存在，拒绝覆盖')
    try:
        for path, data in replacements.items():
            path.write_bytes(data)
        for path, data in additions.items():
            path.write_bytes(data)
        yield
    finally:
        for path, data in originals.items():
            path.write_bytes(data)
        for path in additions:
            path.unlink(missing_ok=True)


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    app = HARMONY / 'AppScope/app.json5'
    extension = SOURCE / 'ets/inputmethodextability/KeyboardExtensionAbility.ets'
    entry = SOURCE / 'ets/entryability/EntryAbility.ets'
    module = SOURCE / 'module.json5'
    mirror = SOURCE / 'ets/keyboard/SharedStateMirror.ets'
    pages = SOURCE / 'resources/base/profile/main_pages.json'
    page_list = json.loads(pages.read_text())
    page_list['src'].append('pages/CanvasProbeEditor')
    extension_text = extension.read_text()
    anchor = 'const files: string = this.context.filesDir;'
    if extension_text.count(anchor) != 1:
        raise RuntimeError('扩展注入点改变，请先审查')
    app_text = app.read_text()
    if app_text.count('app.msime.hmos') != 1:
        raise RuntimeError('应用 ID 注入点改变，请先审查')
    replacements = {
        app: app_text.replace('app.msime.hmos', 'app.msime.hmos.canvasprobe').encode(),
        extension: ("import { CandidateCanvasDeviceFixture } from '../keyboard/CandidateCanvasDeviceFixture';\n"
                    + extension_text.replace(anchor, 'CandidateCanvasDeviceFixture.prepare(this.context);\n      ' + anchor)).encode(),
        entry: entry.read_text().replace("stage.loadContent('pages/Settings'", "stage.loadContent('pages/CanvasProbeEditor'").encode(),
        pages: json.dumps(page_list).encode(),
        # 测试包没有共享沙箱授权，绝不导入或发布原应用的账号和偏好镜像。
        module: module.read_text().replace('"dataGroupIds": ["group.1516753898738609107"]', '"dataGroupIds": []').encode(),
        mirror: mirror.read_text().replace('group.1516753898738609107', 'group.msime.canvasprobe').encode(),
    }
    additions = {
        SOURCE / 'ets/pages/CanvasProbeEditor.ets': (HERE / 'CanvasProbeEditor.ets').read_bytes(),
        SOURCE / 'ets/keyboard/CandidateCanvasDeviceFixture.ets': (HERE / 'CandidateCanvasDeviceFixture.ets').read_bytes(),
    }
    raw_root = SOURCE / 'resources/rawfile/canvas-probe'
    if raw_root.exists():
        raise RuntimeError('测试资源目录已存在，拒绝覆盖')
    try:
        for broken in (False, True):
            name = 'probe-broken' if broken else 'probe-animation'
            directory = raw_root / name
            directory.mkdir(parents=True)
            manifest = f'''schema_version = 2
id = "{name}"
name = "合成画布验收"
version = "1.0.0"
base = "canvas"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
item_gap = "0.3em"
item_padding_right = 0
item_min_width = 72
number_scale = 0
item_corner_radius_dip = 0
[candidate_window.border_image]
source = "panel.apng"
slice_dip = [2]
width_dip = [2]
fill = true
[candidate_window.content_insets_dip]
top = 2
right = 2
bottom = 2
left = 2
[[candidate_window.decorations]]
image = "widget.apng"
anchor = "right"
width_dip = 20
height_dip = 20
z_index = 2
'''
            (directory / 'skin.toml').write_text(manifest)
            panel = apng(8, 8, [(240, 32, 48, 255), (32, 48, 240, 255)])
            if broken:
                # 保留共享扫描器需要的合法图片结构，在像素压缩数据内制造损坏。
                damaged = bytearray(panel)
                at = damaged.index(b'IDAT') + 4
                damaged[at] = 0
                length = struct.unpack('>I', damaged[at - 8:at - 4])[0]
                damaged[at + length:at + length + 4] = struct.pack('>I', zlib.crc32(damaged[at - 4:at + length]))
                panel = bytes(damaged)
            (directory / 'panel.apng').write_bytes(panel)
            (directory / 'widget.apng').write_bytes(apng(4, 4, [(32, 240, 48, 255), (240, 220, 32, 255)]))
        with temporary_sources(replacements, additions):
            subprocess.run(['ohpm', 'install'], cwd=HARMONY, check=True)
            subprocess.run(['hvigorw', 'assembleHap', '--no-daemon'], cwd=HARMONY, check=True)
            shutil.copy2(HARMONY / 'entry/build/default/outputs/default/entry-default-unsigned.hap',
                         OUTPUT / 'canvas-probe.hap')
    finally:
        shutil.rmtree(raw_root)
    print(OUTPUT / 'canvas-probe.hap')


if __name__ == '__main__':
    main()
