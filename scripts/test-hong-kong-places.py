#!/usr/bin/env python3
"""香港地名生成器的行为回归：多音字、字形、原始名称与完整性。"""

import importlib.util
import json
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


if __name__ == '__main__':
    unittest.main()
