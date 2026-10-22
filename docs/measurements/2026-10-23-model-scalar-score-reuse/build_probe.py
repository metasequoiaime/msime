from pathlib import Path
import subprocess, shutil, hashlib, json, struct, os
folder = Path('target/model-score-reuse-evidence').resolve()
resource = Path(os.environ['MSIME_PROBE_RESOURCES'])
with (resource / 'sentence-model.safetensors').open('rb') as stream:
    size = struct.unpack('<Q', stream.read(8))[0]
    header = json.loads(stream.read(size))
vocab = json.loads(header['__metadata__']['vocab'])
(folder / 'vocab.json').write_text(json.dumps(vocab, ensure_ascii=False))
source = Path('crates/engine/src/ordering.rs')
original = source.read_bytes()
example = Path('crates/input-runtime/examples/rerank_latency.rs')
example_original = example.read_bytes()
assert hashlib.sha256(original).hexdigest() == 'bcaeac320fe7c445c29ede14cf2b38efbf641cb60b2930df94b2bd8349005645'
assert hashlib.sha256(example_original).hexdigest() == '359e88acb24476e185c9c9c274cbb89e74994bd894bf13680471fade27a0640a'
for manifest in ['crates/engine/Cargo.toml', 'crates/input-runtime/Cargo.toml']:
    assert 'rev = "15caab4fdeb59c11f452b91884371cf30e2e5608"' in Path(manifest).read_text()
needle = b'            let neural = reranker.log_probabilities(context, &subset[..count]);\n'
assert original.count(needle) == 1
insertion = b'            scalar_probe::observe(probe_id, probe_window, context, &subset[..count], &neural);\n'
before = b'            let probe_id = reranker as *const Reranker as usize;\n            let probe_window = reranker.model().context_length();\n'
try:
    patched = original.replace(needle, before + needle + insertion)
    fallback = b'    with_row_texts(rows, |texts| {\n'
    assert patched.count(fallback) == 1
    patched = patched.replace(fallback, b'    scalar_probe::fallback(reranker as *const Reranker as usize);\n' + fallback)
    patched += ('\n#[path = "' + str(folder / 'scalar_probe.rs') + '"]\nmod scalar_probe;\n').encode()
    source.write_bytes(patched)
    checkpoint = b'    let mut off = Arm::default();\n'
    assert example_original.count(checkpoint) == 1
    example.write_bytes(example_original.replace(checkpoint, b'    eprintln!("scalar-probe-phase,warmup-finished");\n' + checkpoint))
    (folder / 'probe.patch').write_bytes(subprocess.check_output(['git', 'diff', '--', str(source), str(example)]))
    with (folder / 'build.log').open('w') as output:
        subprocess.run(['cargo', 'build', '--locked', '--release', '-p', 'msime-input-runtime', '--example', 'rerank_latency'], stdout=output, stderr=subprocess.STDOUT, check=True)
    shutil.copy2('target/release/examples/rerank_latency', folder / 'probe-rerank_latency')
finally:
    source.write_bytes(original)
    example.write_bytes(example_original)
assert example.read_bytes() == example_original
assert source.read_bytes() == original
(folder / 'binary.json').write_text(json.dumps({'sha256': hashlib.sha256((folder / 'probe-rerank_latency').read_bytes()).hexdigest(), 'source_sha256': hashlib.sha256(original).hexdigest(), 'example_sha256': hashlib.sha256(example_original).hexdigest(), 'source_restored': True}, indent=2) + '\n')
