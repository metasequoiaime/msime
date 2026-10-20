#!/usr/bin/env python3
"""在明确指定的专用模拟器上验收原生候选画布，只操作独立 canvasprobe 包。"""

import argparse
import json
import pathlib
import re
import subprocess
import time

from PIL import Image


BUNDLE = 'app.msime.hmos.canvasprobe'
ROOT = pathlib.Path(__file__).resolve().parents[4]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--hdc', required=True)
    parser.add_argument('--serial', required=True)
    parser.add_argument('--hap', required=True, type=pathlib.Path)
    parser.add_argument('--phases', type=int, choices=(1, 4), default=4)
    args = parser.parse_args()
    output = ROOT / 'target/harmony-tests/device/evidence'
    output.mkdir(parents=True, exist_ok=True)

    def hdc(*arguments):
        return subprocess.run([args.hdc, '-t', args.serial, *arguments], check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30).stdout

    def shell(*arguments):
        return hdc('shell', *arguments)

    def layout():
        remote = '/data/local/tmp/msime-canvas-probe-layout.json'
        shell('uitest', 'dumpLayout', '-b', BUNDLE, '-p', remote)
        path = output / 'layout.json'
        hdc('file', 'recv', remote, str(path))
        result = []

        def visit(node, parents=()):
            if isinstance(node, dict):
                if 'attributes' in node:
                    attributes = node['attributes']
                    result.append({**attributes, 'probeAncestors': parents})
                    parents = (*parents, attributes)
                for key, value in node.items():
                    if key != 'attributes':
                        visit(value, parents)
            elif isinstance(node, list):
                for value in node:
                    visit(value, parents)

        visit(json.loads(path.read_text()))
        return result

    def click(nodes, *, text=None, identifier=None):
        choices = [node for node in nodes if (text is not None and node.get('text') == text)
                   or (identifier is not None and node.get('id') == identifier)]
        if not choices:
            raise AssertionError(f'未找到验收节点：{text or identifier}')
        numbers = [int(value) for value in re.findall(r'\d+', choices[-1]['bounds'])]
        x1, y1, x2, y2 = numbers
        shell('uitest', 'uiInput', 'click', str((x1 + x2) // 2), str((y1 + y2) // 2))

    def capture(label):
        remote = '/data/local/tmp/msime-canvas-probe.png'
        shell('uitest', 'screenCap', '-p', remote)
        path = output / f'{label}.png'
        hdc('file', 'recv', remote, str(path))
        return Image.open(path).convert('RGB')

    original = shell('ime', '-g')
    match = re.search(r'input method is:\s*([\w.]+)', original)
    if match is None:
        raise RuntimeError('不能确定原输入法，拒绝改变选择')
    installed = False
    try:
        result = hdc('install', '-r', str(args.hap.resolve()))
        if 'success' not in result.lower() or 'failed' in result.lower():
            raise RuntimeError(f'测试 HAP 安装失败：{result}')
        installed = True
        shell('ime', '-e', BUNDLE, '-f')
        for phase in range(args.phases):
            if phase > 0:
                shell('ime', '-s', match.group(1))
                shell('aa', 'force-stop', BUNDLE)
            shell('aa', 'start', '-b', BUNDLE, '-a', 'EntryAbility')
            shell('ime', '-s', BUNDLE)
            time.sleep(3)
            click(layout(), identifier='canvasProbeEditor')
            deadline = time.monotonic() + 15
            while True:
                nodes = layout()
                if any(node.get('text') == 'N' for node in nodes):
                    break
                if time.monotonic() >= deadline:
                    raise AssertionError(f'阶段 {phase} 的原生键盘没有就绪')
                time.sleep(0.25)
            phases = re.findall(r'canvas probe phase (\d) ready', shell('hilog', '-z', '100', '-T', 'MSIME'))
            if not phases or int(phases[-1]) != phase:
                raise AssertionError(f'扩展实际阶段与预期 {phase} 不符')
            # 切换输入法会重建扩展，键面可能先于编辑连接就绪；明确收起再聚焦，不能仅凭字母节点开始输入。
            shell('uitest', 'uiInput', 'keyEvent', 'Back')
            time.sleep(0.3)
            click(layout(), identifier='canvasProbeEditor')
            deadline = time.monotonic() + 15
            while True:
                nodes = layout()
                logs = shell('hilog', '-z', '100', '-T', 'MSIME')
                current = logs.rsplit(f'canvas probe phase {phase} ready', 1)[-1]
                if any(node.get('text') == 'N' for node in nodes) and 'attached to editor:' in current:
                    break
                if time.monotonic() >= deadline:
                    raise AssertionError(f'阶段 {phase} 的键盘未连接原生编辑框')
                time.sleep(0.25)
            capture(f'phase-{phase}-before-input')
            for letter in 'nihao':
                click(nodes, text=letter.upper())
            time.sleep(1)
            nodes = layout()
            if not any(node.get('text') == '你好' for node in nodes):
                raise AssertionError('真实系统键盘没有产生合成候选“你好”')
            word = next(node for node in nodes if node.get('text') == '你好' and node.get('type') == 'Text')
            first = next((node for node in reversed(word['probeAncestors'])
                          if node.get('type') == 'Column' and node.get('clickable') == 'true'), None)
            if first is None:
                raise AssertionError('真实候选胶囊没有可访问的布局节点')
            first_bounds = [int(value) for value in re.findall(r'\d+', first['bounds'])]
            chips = [node for node in nodes if node.get('type') == 'Column' and node.get('clickable') == 'true'
                     and node.get('longClickable') == 'true'
                     and int(re.findall(r'\d+', node['bounds'])[1]) == first_bounds[1]]
            chips.sort(key=lambda node: int(re.findall(r'\d+', node['bounds'])[0]))
            if len(chips) < 2:
                raise AssertionError('真实候选胶囊不足两个，无法验收间距')
            bounds = [[int(value) for value in re.findall(r'\d+', chip['bounds'])] for chip in chips]
            editor = next(node for node in nodes if node.get('id') == 'canvasProbeEditor')
            editor_bounds = [int(value) for value in re.findall(r'\d+', editor['bounds'])]
            density = (editor_bounds[3] - editor_bounds[1]) / 200
            if bounds[0][2] - bounds[0][0] < 72 * density - 2:
                raise AssertionError('共享候选最小宽度没有进入触屏组件')
            if bounds[1][0] - bounds[0][2] < 6 * density - 2:
                raise AssertionError('共享候选间距没有进入触屏组件')
            observed = set()
            for index in range(8):
                image = capture(f'phase-{phase}-animation-{index}')
                if index == 0:
                    fill = chips[0].get('backgroundColor', '')
                    if re.fullmatch(r'#FF[0-9A-Fa-f]{6}', fill) is None:
                        raise AssertionError('零圆角像素验收需要不透明的候选底色')
                    expected_corner = tuple(int(fill[at:at + 2], 16) for at in (3, 5, 7))
                    corner = image.getpixel((bounds[0][0] + 1, bounds[0][1] + 1))
                    if any(abs(a - b) > 1 for a, b in zip(corner, expected_corner)):
                        raise AssertionError('显式零圆角没有进入触屏候选绘制')
                colors = dict((rgb, count) for count, rgb in image.getcolors(image.width * image.height))
                for color in ((240, 32, 48), (32, 48, 240), (32, 240, 48), (240, 220, 32)):
                    # 面板必须覆盖候选条内部，不能只有裁切错误留下的一小块角图。
                    threshold = image.width * 20 if phase < 2 and color in ((240, 32, 48), (32, 48, 240)) else 100
                    if colors.get(color, 0) > threshold:
                        observed.add(color)
                time.sleep(0.15)
            expected = {(32, 240, 48), (240, 220, 32)}
            if phase < 2:
                expected |= {(240, 32, 48), (32, 48, 240)}
            if observed != expected:
                raise AssertionError(f'阶段 {phase} 的真实动画帧与回退不符：{sorted(observed)}')
            click(nodes, text='你好')
            time.sleep(0.4)
            if not any(node.get('id') == 'canvasProbeCommit' and node.get('text') == '你好' for node in layout()):
                raise AssertionError('点击候选后没有写入真实 TextArea')
            shell('uitest', 'uiInput', 'keyEvent', 'Back')
            time.sleep(0.3)
            click(layout(), identifier='canvasProbeEditor')
            time.sleep(0.5)
            if not any(node.get('text') == 'N' for node in layout()):
                raise AssertionError('隐藏后重新聚焦没有恢复真实键盘')
            capture(f'phase-{phase}-shown-again')
            print(f'阶段 {phase} 通过：真实候选、面板与挂件帧/回退、点击上屏、隐藏再显示', flush=True)
    except BaseException:
        if installed:
            capture('failure')
        raise
    finally:
        shell('ime', '-s', match.group(1))
        if installed:
            shell('ime', '-d', BUNDLE)
            hdc('uninstall', BUNDLE)
        shell('rm', '-f', '/data/local/tmp/msime-canvas-probe-layout.json', '/data/local/tmp/msime-canvas-probe.png')


if __name__ == '__main__':
    main()
