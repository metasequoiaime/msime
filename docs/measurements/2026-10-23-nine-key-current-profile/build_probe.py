from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

root = Path.cwd()
folder = root / 'target/current-nine-key-evidence'
allocator = root / 'crates/engine/src/ime/personal_rerank.rs'
decode = root / 'crates/engine/src/lattice/decode.rs'
original_allocator = allocator.read_bytes()
original_decode = decode.read_bytes()
allocator_needle = b'    #[global_allocator]\n'
hasher_needle = b'BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.set(calls.get() + 1));'
assert b"fn fill_row" in original_decode, "先应用候选补丁"
assert (root / "crates/engine/src/lattice/decode_row_binding_tests.rs").is_file()
assert original_allocator.count(allocator_needle) == 1
assert original_decode.count(hasher_needle) == 2
sha = lambda data: hashlib.sha256(data).hexdigest()
try:
    allocator.write_bytes(original_allocator.replace(allocator_needle, b'    #[allow(dead_code)]\n'))
    decode.write_bytes(original_decode.replace(hasher_needle, b'{}'))
    with (folder / 'probe-build.jsonl').open('w') as output, (folder / 'probe-build.log').open('w') as errors:
        subprocess.run(['cargo', 'test', '--release', '--locked', '-p', 'msime-engine', '--lib', '--no-run', '--message-format=json'],
                       stdout=output, stderr=errors, check=True, env={**os.environ, 'CARGO_BUILD_JOBS': '4'})
    records = [json.loads(line) for line in (folder / 'probe-build.jsonl').read_text().splitlines()]
    executables = [record['executable'] for record in records if record.get('reason') == 'compiler-artifact' and record.get('target', {}).get('name') == 'msime_engine' and record.get('executable')]
    assert len(executables) == 1
    binary = folder / 'paired-system'
    shutil.copy2(executables[0], binary)
    report = {'allocator': 'std::alloc::System', 'hasher_tls_counter': False,
              'binary_sha256': sha(binary.read_bytes()), 'decode_source_sha256': sha(original_decode),
              'allocator_source_sha256': sha(original_allocator), 'test_decode_source_sha256': sha(decode.read_bytes())}
    (folder / 'probe-binary.json').write_text(json.dumps(report, indent=2) + '\n')
finally:
    allocator.write_bytes(original_allocator)
    decode.write_bytes(original_decode)
assert allocator.read_bytes() == original_allocator and decode.read_bytes() == original_decode
print('普通分配器程序已构建，测试计数器接线已恢复')
