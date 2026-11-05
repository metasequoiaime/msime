#!/usr/bin/env python3
"""香港地名生成器的行为回归：多音字、字形、原始名称与完整性。"""

import importlib.util
import json
import hashlib
import shutil
import subprocess
from unittest.mock import patch
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('hong_kong_places', ROOT / 'crates/dict-builder/hong_kong_places.py')
HK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HK)


class HongKongPlaces(unittest.TestCase):
    def test_polyphone_is_not_guessed(self):
        lexicon = {'示': {'si'}, '行': {'hong', 'hang'}, '路': {'lou'}}
        reading, issues = HK.resolve('示行路', lexicon, {})
        self.assertIsNone(reading)
        self.assertEqual(issues[0]['span'], '行')
        self.assertEqual(HK.resolve('示行路', lexicon, {'示行': 'si hang'}), ('si hang lou', []))

    def test_place_rule_wins_but_single_character_rule_does_not_break_words(self):
        lexicon = {'銀行': {'ngan hong'}, '行': {'hong', 'hang'}, '路': {'lou'}}
        self.assertEqual(HK.resolve('銀行路', lexicon, {'行': 'hang'}), ('ngan hong lou', []))
        lexicon['示行路'] = {'si hong lou'}
        self.assertEqual(HK.resolve('示行路', lexicon, {'示行': 'si hang'}), ('si hang lou', []))

    def test_no_silent_removal_of_letter_or_private_character(self):
        self.assertEqual(HK.normalize_name(' 示範路　'), '示範路')
        self.assertEqual(HK.normalize_name('示範公路　－　示例段'), '示範公路示例段')
        self.assertEqual(HK.normalize_name('示範村 (北)'), '示範村北')
        self.assertEqual(HK.normalize_name('示範圍１巷'), '示範圍一巷')
        for word in ['示範Ｂ街', '示\uf4b0路']:
            self.assertEqual(HK.normalize_name(word), word)
            self.assertFalse(HK.han(word))
        self.assertTrue(HK.han('䃟頭'))

    def test_snapshot_corruption_and_incomplete_source_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sources = root / HK.DIRECTORY
            sources.mkdir(parents=True)
            manifest = {'files': []}
            (sources / 'sources.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, '完整'):
                HK.names_from_snapshot(root)
            for path in ['landsd-place-names.geojson', 'landsd-street-names.json', 'CSDI-terms.txt']:
                (sources / path).write_text('[]')
                manifest['files'].append({'path': path, 'size': 2, 'sha256': '0' * 64})
            (sources / 'sources.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, '摘要'):
                HK.names_from_snapshot(root)

    def test_new_character_needs_explicit_source_but_existing_reading_cannot_be_bypassed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'curation').mkdir()
            rules = root / HK.RULES
            lexicon = {'yue': {}, 'cmn': {'行': {'hang', 'xing'}}}
            rules.write_text('cmn\t䃟\tsan\n')
            with self.assertRaises(ValueError):
                HK.load_rules(root, lexicon)
            rules.write_text('cmn\t䃟\tsan\thttps://example.invalid/character\n')
            self.assertEqual(HK.load_rules(root, lexicon)['cmn']['䃟'], 'san')
            rules.write_text('cmn\t行\tsan\thttps://example.invalid/character\n')
            with self.assertRaises(ValueError):
                HK.load_rules(root, lexicon)

    def generated_fixture(self, root):
        for path in HK.INPUT_PATHS:
            output = root / path
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text('合成输入\n')
        rows = ["示例街\t示例街\tsi lai gaai\tshi'li'jie"]
        audit = {'ready': ['示例街'], 'pending': [], 'unsupported': [],
                 'counts': {'ready': 1, 'pending': 0, 'unsupported': 0}}
        table = ('# msime 仓库提交 ' + 'a' * 40 + '\n' + '\n'.join(rows) + '\n').encode()
        report = json.dumps(audit).encode()
        (root / HK.OUTPUT).write_bytes(table)
        (root / HK.DIRECTORY / 'audit.json').write_bytes(report)
        manifest = {'generated': {'path': HK.OUTPUT.name, 'msime_commit': 'a' * 40,
                    'size': len(table), 'sha256': hashlib.sha256(table).hexdigest(),
                    'audit_sha256': hashlib.sha256(report).hexdigest(), 'inputs': HK.input_records(root),
                    'rows': 1, 'counts': audit['counts']}}
        (root / HK.DIRECTORY / 'sources.json').write_text(json.dumps(manifest))
        return rows, audit

    def test_check_binds_every_generation_input_and_both_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows, audit = self.generated_fixture(root)
            HK.check_generated(root, rows, audit)
            for path in [*HK.INPUT_PATHS, HK.OUTPUT, HK.DIRECTORY / 'audit.json']:
                with self.subTest(path=path):
                    target = root / path
                    original = target.read_bytes()
                    target.write_bytes(original + b'changed')
                    with self.assertRaises(ValueError):
                        HK.check_generated(root, rows, audit)
                    target.write_bytes(original)

    def test_check_does_not_repair_a_corrupt_audit_before_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows, audit = self.generated_fixture(root)
            report = root / HK.DIRECTORY / 'audit.json'
            report.write_text('damaged')
            with patch.object(HK, 'generate', return_value=(rows, audit)), patch('sys.argv', [
                'hong_kong_places.py', '--dictionary', str(root), '--audit', str(report), '--check'
            ]), self.assertRaises(ValueError):
                HK.main()
            self.assertEqual(report.read_text(), 'damaged')

    def test_check_preserves_valid_saved_audit_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows, audit = self.generated_fixture(root)
            report = root / HK.DIRECTORY / 'audit.json'
            original = report.read_bytes()
            with patch.object(HK, 'generate', return_value=(rows, audit)), patch('sys.argv', [
                'hong_kong_places.py', '--dictionary', str(root), '--audit', str(report), '--check'
            ]):
                HK.main()
            self.assertEqual(report.read_bytes(), original)

    @unittest.skipUnless(shutil.which('git'), '需要 Git 验证生成器提交归属')
    def test_generator_must_exist_unchanged_in_the_recorded_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return subprocess.check_output(['git', '-c', 'core.hooksPath=/dev/null',
                    '-c', 'user.name=Synthetic', '-c', 'user.email=synthetic@example.invalid',
                    *args], cwd=root, stderr=subprocess.DEVNULL)
            git('init', '-q')
            (root / 'README').write_text('合成仓库')
            git('add', 'README')
            git('commit', '--no-gpg-sign', '-qm', 'fixture')
            script = root / 'generator.py'
            script.write_text('# 合成生成器\n')
            with self.assertRaisesRegex(ValueError, '尚未提交'):
                HK.generator_revision(root, script)
            git('add', 'generator.py')
            git('commit', '--no-gpg-sign', '-qm', 'generator')
            self.assertEqual(HK.generator_revision(root, script), git('rev-parse', 'HEAD').decode().strip())
            script.write_text('# 已修改\n')
            with self.assertRaisesRegex(ValueError, '未提交改动'):
                HK.generator_revision(root, script)


if __name__ == '__main__':
    unittest.main()
