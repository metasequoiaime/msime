#!/usr/bin/env python3
"""合成 adb 验证设备单测程序的退出清理与错误保留。"""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / 'platforms/android/tests/device/run-core-test.sh'
REMOTE = '/data/local/tmp/msime-rust-tests/core-tests'


class CoreRunnerCleanupTests(unittest.TestCase):
    def run_case(self, mode):
        with tempfile.TemporaryDirectory(prefix='msime-core-runner-') as directory:
            root = Path(directory)
            adb = root / 'platform-tools/adb'
            adb.parent.mkdir()
            adb.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
args = sys.argv[3:]
with Path(os.environ['ADB_CALLS']).open('a') as output:
    output.write(json.dumps(args) + '\\n')
mode = os.environ['ADB_MODE']
if args == ['emu', 'avd', 'name']:
    print('personal-phone' if mode == 'wrong-avd' else 'msime-client-test-api37')
elif args == ['shell', 'getprop', 'sys.boot_completed']:
    print('1')
elif args[0] == 'push' and mode == 'push-failure':
    sys.exit(23)
elif args[:3] == ['shell', 'rm', '-f'] and mode == 'cleanup-failure':
    sys.exit(24)
elif args[0] == 'shell' and args[1].startswith('TMPDIR='):
    sys.exit(17 if mode == 'test-failure' else 0)
''')
            adb.chmod(0o755)
            binary = root / 'synthetic-test'
            binary.write_text('synthetic')
            calls_path = root / 'calls.jsonl'
            result = subprocess.run(
                ['bash', str(RUNNER), str(binary), 'synthetic::case'],
                env={**os.environ, 'ANDROID_SDK_ROOT': str(root),
                     'MSIME_ANDROID_TEST_SERIAL': 'emulator-5584',
                     'ADB_CALLS': str(calls_path), 'ADB_MODE': mode},
                capture_output=True, text=True, timeout=10)
            return result.returncode, [json.loads(line) for line in calls_path.read_text().splitlines()]

    def test_success_and_failure_clean_only_the_uploaded_binary(self):
        for mode, expected in [('success', 0), ('test-failure', 17),
                               ('push-failure', 23), ('cleanup-failure', 0)]:
            with self.subTest(mode=mode):
                status, calls = self.run_case(mode)
                self.assertEqual(status, expected)
                self.assertEqual([call for call in calls if call[:2] == ['shell', 'rm']],
                                 [['shell', 'rm', '-f', REMOTE]])
                self.assertEqual(calls[-1], ['shell', 'rm', '-f', REMOTE])
                if mode != 'push-failure':
                    self.assertIn(['shell', f'TMPDIR=/data/local/tmp/msime-rust-tests {REMOTE} synthetic::case --skip hard_link'], calls)

    def test_non_test_device_is_not_modified(self):
        status, calls = self.run_case('wrong-avd')
        self.assertNotEqual(status, 0)
        self.assertEqual(calls, [['emu', 'avd', 'name']])


if __name__ == '__main__':
    unittest.main()
