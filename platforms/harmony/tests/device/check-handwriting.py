#!/usr/bin/env python3
"""在水杉应用内空白试用框验收真实手写、选字、删除和重写，不发送消息。"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

BUNDLE = 'app.msime.hmos'
CANVAS = 'msime-harmony-handwriting-canvas'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--hdc', required=True)
    parser.add_argument('--serial', required=True)
    args = parser.parse_args()
    remote = f'/data/local/tmp/msime-handwriting-check-{os.getpid()}.json'

    def hdc(*arguments):
        return subprocess.check_output([args.hdc, '-t', args.serial, *arguments],
                                       text=True, timeout=30)

    def bounds(node):
        return tuple(map(int, re.findall(r'-?\d+', node['bounds'])))

    with tempfile.TemporaryDirectory(prefix='msime-handwriting-') as directory:
        local = Path(directory) / 'layout.json'

        def layout():
            hdc('shell', 'uitest', 'dumpLayout', '-b', BUNDLE, '-p', remote)
            hdc('file', 'recv', remote, str(local))
            nodes = []

            def visit(node):
                nodes.append(node.get('attributes', {}))
                for child in node.get('children', []):
                    visit(child)

            visit(json.loads(local.read_text()))
            assert any(n.get('text') == 'resource:/RAWFILE/settings/index.html' for n in nodes), \
                '水杉应用不在前台，停止操作'
            assert any(n.get('hint') == '聊天消息 打几个字试试' for n in nodes), \
                '请先打开应用内的空白试用键盘页面'
            return nodes

        def editor(nodes):
            return next(n for n in nodes if n.get('hint') == '聊天消息 打几个字试试')

        def click(node):
            left, top, right, bottom = bounds(node)
            hdc('shell', 'uitest', 'uiInput', 'click', str((left + right) // 2),
                str((top + bottom) // 2))

        def reset():
            click(next(n for n in layout() if n.get('text') == '重写'))

        def wait_for(predicate):
            deadline = time.monotonic() + 20
            while True:
                nodes = layout()
                if predicate(nodes):
                    return nodes
                assert time.monotonic() < deadline, '手写验收等待超时'
                time.sleep(0.3)

        def write():
            nodes = layout()
            assert editor(nodes).get('text', '') == '', '试用框不是空白，停止操作'
            left, top, right, bottom = bounds(next(n for n in nodes if n.get('id') == CANVAS))
            width, height = right - left, bottom - top
            # 合成「工」的三笔通过系统触摸注入，经过产品的真实触摸和停笔识别路径。
            for x1, y1, x2, y2 in ((.24, .25, .78, .25), (.51, .25, .51, .80),
                                    (.21, .80, .81, .80)):
                layout()
                hdc('shell', 'uitest', 'uiInput', 'swipe',
                    str(round(left + width * x1)), str(round(top + height * y1)),
                    str(round(left + width * x2)), str(round(top + height * y2)), '1000')
            return wait_for(lambda ns: any(n.get('text') == '工' for n in ns))

        try:
            nodes = layout()
            assert editor(nodes).get('text', '') == '', '试用框不是空白，停止操作'
            assert any(n.get('id') == CANVAS for n in nodes), '请先切换到手写方案'
            reset()
            nodes = write()
            click(next(n for n in nodes if n.get('text') == '工'))
            nodes = wait_for(lambda ns: editor(ns).get('text') == '工')
            assert any(n.get('text') == '在此手写，停笔后选字' for n in nodes), '选字后笔迹未清空'
            canvas = next(n for n in nodes if n.get('id') == CANVAS)
            rewrite = next(n for n in nodes if n.get('text') == '重写')
            _, top, _, bottom = bounds(canvas)
            left, _, right, _ = bounds(rewrite)
            hdc('shell', 'uitest', 'uiInput', 'click', str((left + right) // 2),
                str(round(top + (bottom - top) / 4)))
            wait_for(lambda ns: editor(ns).get('text', '') == '')
            write()
            reset()
            nodes = wait_for(lambda ns: any(n.get('text') == '在此手写，停笔后选字' for n in ns))
            assert not any(n.get('text') == '工' for n in nodes), '重写后仍残留候选'
            assert editor(nodes).get('text', '') == '', '验收结束时试用框未清空'
            print('手写真机验收通过：真实笔画识别、选字上屏、空画布删除和重写清空。')
        finally:
            hdc('shell', 'rm', remote)


if __name__ == '__main__':
    main()
