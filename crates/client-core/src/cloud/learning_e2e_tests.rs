use super::*;
use std::collections::BTreeMap;

// ---- 已知答案 ----

// 由独立实现（pyca/cryptography 的 X25519、HKDF 和 ChaCha20-Poly1305，加上按 draft-irtf-cfrg-xchacha 手写的 HChaCha20，并先用它复现了云剪贴板已发布的密文）按 docs/learning-sync.md 算出，不是从这个模块的输出抄来的。设备 A 的私钥是 32 个 0x01，设备 D 的是 32 个 0x07，账号密钥是 32 个 0x02；封装的临时私钥是 32 个 0x04、nonce 是 24 个 0x05；信封的 nonce 是 24 个 0x08、代次 3、上传设备是 D。
const PUBLIC_KEY_A: &str = "pOCSkrZRwni5dyxWn1-puxPZBrRqtoyd-dwrRAn4ogk";
const DEVICE_ID_A: &str = "1a92f23852dc908d97316a3b13578281";
const CODE_A: &str = "3799 5882 9735 3363";
const PUBLIC_KEY_D: &str = "E75P6uryBMf9M1j8nAByGIHRdCeBKCJ-xnTzf3_pe20";
const DEVICE_ID_D: &str = "0f6d2553ff0c0f447ae202edd84383e1";
const CODE_D: &str = "6259 5760 4272 2385";
const KEY_ID: &str = "623094f752e30f5a8cc9ca3b612d1a42";
const WRAP_A_TO_D: &str = "v1.rAGyIJ6GNU-4UyN7XeD0-rE8f8v0M6YcAZNpYX_s8QsFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQVu_f63T8UUJpDx6zevpyd4UNndmg8QFgL-vBwLoqPai2ay4NWNLzGBww3XdCKi4e4";
const WRAP_A_TO_A: &str = "v1.rAGyIJ6GNU-4UyN7XeD0-rE8f8v0M6YcAZNpYX_s8QsFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQXW0pwXRSZlJOFRKsZ6XxYObpdYfxtFvgziMYjGqRWnUJuBI1mqR6XYRaRvQ8IRe4Q";
const REVERSE_CODE_A_TO_D: &str = "5876 2669 8663 8335";
const STREAM: &str = concat!(
    "{\"type\":\"header\",\"format\":\"msime-learning-sync\",\"version\":1,\"content_id\":\"kat\"}\n",
    "{\"type\":\"weight\",\"dictionary\":\"pinyin\",\"key\":\"ni'hao\",\"value\":\"你好\",\"weight\":12345,\"display\":\"\",\"updated_at\":1793000000}\n",
    "{\"type\":\"footer\",\"records\":1}\n",
);
// Python `gzip.compress(STREAM, compresslevel=9, mtime=0)`。
const GZIP_HEX: &str = "1f8b08000000000002ff4d8f510e82300c86df39c65e7c81444463e43266d90a3440b76c43b310eee28bc7e0367a0f3ba2897debfff7fbdbce22440ba2161d480d4ee4a2316e948195d1e308c500d211525bf8488aed1b388f86445de642190a40e18a9ac77b86966cfee5dd01db2e30a0510506a48bac5aa488c46a0fa925dc75d2a454394c897aad8ff77365e18bd7e5a13a9e5288b7834c087b93d53280bea62bcbf3a5da6ff5b7bb31266cbf3850c669cf634bf6011af4c939ea000000";
const ENVELOPE_HEX: &str = "4d534c31623094f752e30f5a8cc9ca3b612d1a420f6d2553ff0c0f447ae202edd84383e10000000000000003080808080808080808080808080808080808080808080808f8a7d35d60b80dc816bfd575d90ebc3d14509834ee434e271805ea9d46f8320a0d42e505e843eb8604e75c57fb5c15e6a7749ef55f719842eb1ca582d84271aae33d49a9926b078a2bc9d5270160accc4e7df9b0cbfb87c34bde4fd9324577fc781f6bb5f011b4ec9bf944b6c4c70e0e5af741dec72728d5e2ac832e0f67908c250ec75fb0bb95ee1e4f0e20dfb9845c20bc58d5934c5be20864044e8ef97fa73ffc205572fd3ea98b2b5fbe81d8ba1d958be552d92672fbcd31e5da065bb93acc830d25c83b49899fad9d67a2235583c361dc413d6309ea4f1ea616b8036b02";

fn fixed_key() -> LearningKey {
    LearningKey::new([2u8; 32])
}

fn secret(byte: u8) -> StaticSecret {
    StaticSecret::from([byte; 32])
}

fn public(secret: &StaticSecret) -> [u8; 32] {
    PublicKey::from(secret).to_bytes()
}

fn raw_device(public_key: &[u8; 32]) -> [u8; 16] {
    hex::decode(device_id(public_key))
        .unwrap()
        .try_into()
        .unwrap()
}

fn confirmed(secret_byte: u8, key: LearningKey) -> Keystore {
    let secret = secret(secret_byte);
    let public_key = public(&secret);
    Keystore {
        secret,
        key: Some(HeldKey::own(key, public_key)),
    }
}

#[test]
fn known_answer_vectors_pin_the_wire_format() {
    let a = secret(1);
    let d = secret(7);
    let (pa, pd) = (public(&a), public(&d));
    assert_eq!(B64.encode(pa), PUBLIC_KEY_A);
    assert_eq!(device_id(&pa), DEVICE_ID_A);
    assert_eq!(verification_code(&pa), CODE_A);
    assert_eq!(B64.encode(pd), PUBLIC_KEY_D);
    assert_eq!(device_id(&pd), DEVICE_ID_D);
    assert_eq!(verification_code(&pd), CODE_D);

    let key = fixed_key();
    assert_eq!(key.id, KEY_ID);
    assert_eq!(
        wrap_with(&key, &a, &pd, [4u8; 32], [5u8; 24]).unwrap(),
        WRAP_A_TO_D
    );
    assert_eq!(
        wrap_with(&key, &a, &pa, [4u8; 32], [5u8; 24]).unwrap(),
        WRAP_A_TO_A
    );
    assert_eq!(WRAP_A_TO_D.len(), 142);
    assert_eq!(
        unwrap_key(&d, KEY_ID, WRAP_A_TO_D, &pa).unwrap().bytes,
        [2u8; 32]
    );
    assert_eq!(
        unwrap_key(&a, KEY_ID, WRAP_A_TO_A, &pa).unwrap().bytes,
        [2u8; 32]
    );
    assert_eq!(reverse_code(&key, &pa, &pd), REVERSE_CODE_A_TO_D);

    let gz = hex::decode(GZIP_HEX).unwrap();
    let envelope = seal_compressed_with(&key, raw_device(&pd), 3, &gz, [8u8; 24]).unwrap();
    assert_eq!(hex::encode(&envelope), ENVELOPE_HEX);
    let opened = open_with(&key, DEVICE_ID_D, &envelope).unwrap();
    assert_eq!(opened.generation, 3);
    assert_eq!(opened.stream, STREAM.as_bytes());
}

#[test]
fn clipboard_and_learning_labels_never_collide() {
    // 同一个公钥在两个域里的核对码不同，同一把 32 字节密钥在两个域里的编号不同。
    let pa = public(&secret(1));
    assert_ne!(
        verification_code(&pa),
        super::super::clipboard_e2e::verification_code(&pa)
    );
    assert_ne!(fixed_key().id, "adbf2e90f1a25f07c0ed00dab2fe0111");
}

#[test]
fn padme_matches_the_reference_and_is_a_fixed_point() {
    // 参考值同样由上面的 Python 实现算出。
    for (length, padded) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 3),
        (4, 4),
        (5, 5),
        (7, 7),
        (8, 8),
        (9, 10),
        (15, 16),
        (16, 16),
        (17, 18),
        (100, 104),
        (1000, 1024),
        (1023, 1024),
        (1024, 1024),
        (1025, 1088),
        (4_300_000, 4_325_376),
        (8_388_524, 8_388_608),
    ] {
        assert_eq!(padme(length), padded, "padme({length})");
    }
    for length in (0..5000).chain([65_535, 65_536, 1 << 20, (1 << 23) - 84]) {
        let padded = padme(length);
        assert!(padded >= length);
        assert_eq!(padme(padded), padded, "padme({length}) is a fixed point");
        if length >= 64 {
            assert!(
                padded - length <= length * 12 / 100,
                "overhead for {length}"
            );
        }
    }
}

// ---- 信封 ----

#[test]
fn sealed_blobs_round_trip_and_only_open_for_their_device_and_key() {
    let a = confirmed(1, fixed_key());
    let b = confirmed(7, fixed_key());
    let envelope = a.seal_blob(9, STREAM.as_bytes()).unwrap();
    assert_eq!(&envelope[..4], b"MSL1");
    let padded = (envelope.len() - HEADER_BYTES - TAG_BYTES) as u64;
    assert_eq!(padme(padded), padded);
    // 任何一台持有同一把密钥的设备都能解开 A 的那份，但只能按 A 的编号请求。
    let opened = b.open_blob(&a.device_id(), &envelope).unwrap();
    assert_eq!(opened.generation, 9);
    assert_eq!(opened.stream, STREAM.as_bytes());
    assert!(b.open_blob(&b.device_id(), &envelope).is_none());
    // 另一把密钥解不开。
    let other = confirmed(7, LearningKey::new([9u8; 32]));
    assert!(other.open_blob(&a.device_id(), &envelope).is_none());
    // 两次加密的 nonce 不同。
    assert_ne!(a.seal_blob(9, STREAM.as_bytes()).unwrap(), envelope);
    // 空的明文流也是合法的信封。
    let empty = a.seal_blob(1, b"").unwrap();
    assert_eq!(b.open_blob(&a.device_id(), &empty).unwrap().stream, b"");
}

#[test]
fn tampering_with_the_authenticated_header_fails_to_open() {
    let key = fixed_key();
    let pd = public(&secret(7));
    let gz = hex::decode(GZIP_HEX).unwrap();
    let envelope = seal_compressed_with(&key, raw_device(&pd), 3, &gz, [8u8; 24]).unwrap();
    assert!(open_with(&key, DEVICE_ID_D, &envelope).is_some());

    // 代次换成另一个合法值：头部检查通过，AEAD 不通过。
    let mut generation = envelope.clone();
    generation[43] = 4;
    assert!(open_with(&key, DEVICE_ID_D, &generation).is_none());

    // 设备编号换成别的设备，并按那台设备请求：头部检查通过，AEAD 不通过。
    let mut device = envelope.clone();
    let other = raw_device(&public(&secret(1)));
    device[20..36].copy_from_slice(&other);
    assert!(open_with(&key, DEVICE_ID_A, &device).is_none());

    // 密钥编号换掉，并用同一把密钥、按那个编号去解：AEAD 不通过。
    let mut key_id = envelope.clone();
    key_id[4..20].copy_from_slice(&[0xab; 16]);
    let relabelled = LearningKey {
        id: hex::encode([0xab; 16]),
        raw_id: [0xab; 16],
        bytes: key.bytes,
    };
    assert!(open_with(&relabelled, DEVICE_ID_D, &key_id).is_none());
    assert!(open_with(&key, DEVICE_ID_D, &key_id).is_none());

    // nonce、密文和 tag 的任何一位。
    for index in [50, HEADER_BYTES, envelope.len() - 1] {
        let mut flipped = envelope.clone();
        flipped[index] ^= 1;
        assert!(
            open_with(&key, DEVICE_ID_D, &flipped).is_none(),
            "byte {index}"
        );
    }

    // 魔数、截断、多出字节。
    let mut magic = envelope.clone();
    magic[3] = b'2';
    assert!(open_with(&key, DEVICE_ID_D, &magic).is_none());
    assert!(open_with(&key, DEVICE_ID_D, &envelope[..envelope.len() - 1]).is_none());
    let mut longer = envelope.clone();
    longer.push(0);
    assert!(open_with(&key, DEVICE_ID_D, &longer).is_none());
    assert!(open_with(&key, DEVICE_ID_D, &envelope[..HEADER_BYTES]).is_none());
}

#[test]
fn generations_outside_the_range_are_refused_both_ways() {
    let a = confirmed(1, fixed_key());
    assert_eq!(a.seal_blob(0, b""), Err(SealError::InvalidGeneration));
    assert_eq!(
        a.seal_blob(MAX_GENERATION + 1, b""),
        Err(SealError::InvalidGeneration)
    );
    assert!(a.seal_blob(MAX_GENERATION, b"").is_ok());
    let key = fixed_key();
    let device = raw_device(&a.public_key());
    let gz = gzip(b"");
    for generation in [0, MAX_GENERATION + 1, u64::MAX] {
        let envelope = seal_compressed_with(&key, device, generation, &gz, [1u8; 24]).unwrap();
        assert!(open_with(&key, &a.device_id(), &envelope).is_none());
    }
}

#[test]
fn padding_and_length_field_are_checked_after_decryption() {
    let key = fixed_key();
    let pa = public(&secret(1));
    let device = raw_device(&pa);
    let id = device_id(&pa);
    let gz = gzip(STREAM.as_bytes());
    let length = padme(4 + gz.len() as u64) as usize;
    let build = |padded: &[u8]| seal_padded_with(&key, device, 1, padded, [6u8; 24]);
    let mut good = (gz.len() as u32).to_be_bytes().to_vec();
    good.extend_from_slice(&gz);
    good.resize(length, 0);
    assert!(open_with(&key, &id, &build(&good)).is_some());

    // 补齐里有非零字节。
    if length > 4 + gz.len() {
        let mut dirty = good.clone();
        *dirty.last_mut().unwrap() = 1;
        assert!(open_with(&key, &id, &build(&dirty)).is_none());
    }
    // 长度字段越界。
    let mut overflow = good.clone();
    overflow[..4].copy_from_slice(&(length as u32).to_be_bytes());
    assert!(open_with(&key, &id, &build(&overflow)).is_none());
    // 补齐后的长度不是 Padmé 的不动点。
    let mut odd = gz.clone();
    odd.splice(0..0, (gz.len() as u32).to_be_bytes());
    while padme(odd.len() as u64) == odd.len() as u64 {
        odd.push(0);
    }
    assert!(open_with(&key, &id, &build(&odd)).is_none());
}

#[test]
fn only_a_single_gzip_member_with_nothing_after_it_is_accepted() {
    let key = fixed_key();
    let pa = public(&secret(1));
    let device = raw_device(&pa);
    let id = device_id(&pa);
    let seal = |compressed: &[u8]| seal_compressed_with(&key, device, 1, compressed, [6u8; 24]);

    let single = gzip(b"one");
    assert_eq!(
        open_with(&key, &id, &seal(&single).unwrap())
            .unwrap()
            .stream,
        b"one"
    );
    let mut two = single.clone();
    two.extend_from_slice(&gzip(b"two"));
    assert!(open_with(&key, &id, &seal(&two).unwrap()).is_none());
    let mut trailing = single.clone();
    trailing.push(7);
    assert!(open_with(&key, &id, &seal(&trailing).unwrap()).is_none());
    assert!(open_with(&key, &id, &seal(b"not gzip").unwrap()).is_none());
    let mut corrupt = single.clone();
    let crc = corrupt.len() - 8;
    corrupt[crc] ^= 1;
    assert!(open_with(&key, &id, &seal(&corrupt).unwrap()).is_none());
}

#[test]
fn the_gzip_header_carries_no_time_or_name() {
    let gz = gzip(STREAM.as_bytes());
    assert_eq!(
        &gz[..4],
        &[0x1f, 0x8b, 8, 0],
        "deflate, no FNAME/FCOMMENT/FEXTRA"
    );
    assert_eq!(&gz[4..8], &[0, 0, 0, 0], "MTIME is zero");
    assert_eq!(gunzip(&gz).unwrap(), STREAM.as_bytes());
}

#[test]
fn decompression_stops_at_the_stream_limit() {
    // 全零的流压得极小，正好用来造一个解压后超过上限的 gzip；上限换成 1 MiB 测，免得测试里压 128 MiB。
    let limit = 1 << 20;
    let bomb = gzip(&vec![0u8; limit + 1]);
    assert!(bomb.len() < 4096);
    assert!(gunzip_at_most(&bomb, limit).is_none());
    assert_eq!(
        gunzip_at_most(&gzip(&vec![0u8; limit]), limit)
            .unwrap()
            .len(),
        limit
    );
    let a = confirmed(1, fixed_key());
    assert_eq!(
        a.seal_blob(1, &vec![0u8; MAX_STREAM_BYTES + 1]),
        Err(SealError::TooLarge)
    );
}

#[test]
fn an_envelope_over_eight_mebibytes_is_refused() {
    let key = fixed_key();
    let device = raw_device(&public(&secret(1)));
    // 8 MiB 减去头和 tag 是 8,388,524 字节，不是 Padmé 的不动点（会补到 8,388,608）；不超过它的最大不动点是 63 × 2¹⁷ = 8,257,536。所以压缩后的字节最多 8,257,532 个，信封最大 8,257,620 字节。
    let largest = 63 << 17;
    assert_eq!(padme(largest as u64), largest as u64);
    assert!(padme(largest as u64 + 1) as usize + HEADER_BYTES + TAG_BYTES > MAX_ENVELOPE_BYTES);
    let fits = largest - 4;
    let envelope = seal_compressed_with(&key, device, 1, &vec![0u8; fits], [1u8; 24]).unwrap();
    assert_eq!(envelope.len(), largest + HEADER_BYTES + TAG_BYTES);
    assert!(seal_compressed_with(&key, device, 1, &vec![0u8; fits + 1], [1u8; 24]).is_none());

    // 随机字节压不小，seal_blob 报 TooLarge。
    let mut noise = vec![0u8; MAX_ENVELOPE_BYTES];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for byte in &mut noise {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }
    let a = confirmed(1, fixed_key());
    assert_eq!(a.seal_blob(1, &noise), Err(SealError::TooLarge));
}

// ---- 封装与反向核对 ----

#[test]
fn wraps_open_only_with_the_right_approver_recipient_and_key_id() {
    let key = fixed_key();
    let (a, d, x) = (secret(1), secret(7), secret(9));
    let (pa, pd, px) = (public(&a), public(&d), public(&x));
    let wrap = wrap_key(&key, &a, &pd).unwrap().unwrap();
    assert_eq!(wrap.len(), 142);
    assert_eq!(
        unwrap_key(&d, &key.id, &wrap, &pa).unwrap().bytes,
        key.bytes
    );
    // 服务端谎报批准者：AEAD 不通过。
    assert!(unwrap_key(&d, &key.id, &wrap, &px).is_none());
    // 不是给这台设备的。
    assert!(unwrap_key(&x, &key.id, &wrap, &pa).is_none());
    // 编号不对。
    assert!(unwrap_key(&d, &"0".repeat(32), &wrap, &pa).is_none());
    assert!(unwrap_key(&d, &key.id, &wrap[..141], &pa).is_none());
    // 低阶点的公钥得不出共享密钥。
    assert!(wrap_with(&key, &a, &[0u8; 32], [4u8; 32], [5u8; 24]).is_none());
    assert!(unwrap_key(&d, &key.id, &wrap, &[0u8; 32]).is_none());
}

#[test]
fn a_forged_approver_shows_a_different_reverse_code() {
    // 真设备 A 用 K 批准 D；服务端自己登记的假批准者 F 用它造的 K' 封装给 D。D 解得开 F 的封装（带认证的封装只证明出自持有 F 私钥的一方），但两边的反向核对码对不上。
    let (a, d, f) = (secret(1), secret(7), secret(9));
    let (pa, pd, pf) = (public(&a), public(&d), public(&f));
    let genuine = fixed_key();
    let forged = LearningKey::new([3u8; 32]);
    let shown_on_a = reverse_code(&genuine, &pa, &pd);
    let wrap = wrap_key(&forged, &f, &pd).unwrap().unwrap();
    let opened = unwrap_key(&d, &forged.id, &wrap, &pf).unwrap();
    assert_ne!(reverse_code(&opened, &pf, &pd), shown_on_a);
    // 即使 F 冒用 A 的公钥作为批准者，D 也解不开（F 没有 A 的私钥）。
    let impostor = wrap_key(&forged, &f, &pd).unwrap().unwrap();
    assert!(unwrap_key(&d, &forged.id, &impostor, &pa).is_none());
}

// ---- 密钥串 ----

#[test]
fn keystore_round_trips_every_state_and_rejects_tampering() {
    let mut keystore = Keystore::generate().unwrap();
    let bare = keystore.encode();
    assert_eq!(bare.split('.').count(), 2);
    assert!(Keystore::parse(&bare).unwrap().ready_key_id().is_none());

    let approver = public(&secret(1));
    for state in [
        KeyState::Confirmed,
        KeyState::Unconfirmed,
        KeyState::Rejected,
    ] {
        keystore.key = Some(HeldKey {
            id: KEY_ID.to_owned(),
            key: (state != KeyState::Rejected).then(fixed_key),
            approver,
            state,
        });
        let encoded = keystore.encode();
        let parsed = Keystore::parse(&encoded).unwrap();
        assert_eq!(parsed.encode(), encoded);
        let held = parsed.key.as_ref().unwrap();
        assert_eq!(held.state, state);
        assert_eq!(held.approver, approver);
        assert_eq!(
            parsed.ready_key_id(),
            (state == KeyState::Confirmed).then_some(KEY_ID)
        );
    }

    let parts = |state: &str, key: &str| {
        let secret = bare.strip_prefix("v1.").unwrap();
        format!(
            "v1.{secret}.{KEY_ID}.{key}.{}.{state}",
            B64.encode(approver)
        )
    };
    let key = B64.encode([2u8; 32]);
    assert!(Keystore::parse(&parts("confirmed", &key)).is_some());
    assert!(Keystore::parse(&parts("confirmed", "-")).is_none());
    assert!(Keystore::parse(&parts("rejected", &key)).is_none());
    assert!(Keystore::parse(&parts("approved", &key)).is_none());
    assert!(Keystore::parse(&parts("confirmed", &B64.encode([3u8; 32]))).is_none());
    assert!(Keystore::parse(&format!("{}.extra", parts("confirmed", &key))).is_none());
    assert!(Keystore::parse(&format!("{bare}.{KEY_ID}")).is_none());
    assert!(Keystore::parse("v2.AAAA").is_none());
    assert!(Keystore::parse("").is_none());
}

#[test]
fn only_a_confirmed_key_seals_or_opens() {
    let mut keystore = confirmed(1, fixed_key());
    let envelope = keystore.seal_blob(1, b"x").unwrap();
    keystore.key.as_mut().unwrap().state = KeyState::Unconfirmed;
    assert_eq!(keystore.seal_blob(1, b"x"), Err(SealError::NotReady));
    assert!(keystore
        .open_blob(&keystore.device_id(), &envelope)
        .is_none());
    keystore.key = None;
    assert_eq!(keystore.seal_blob(1, b"x"), Err(SealError::NotReady));
}

// ---- 按 docs/learning-sync.md 和 msime-cloud 的 learning_e2e.go 实现的内存服务端 ----

struct FakeDevice {
    device_id: String,
    public_key: String,
    name: String,
    session: String,
}

struct FakeWrap {
    key_id: String,
    wrap: String,
    approver: String,
}

#[derive(Default)]
struct FakeServer {
    key_id: Option<String>,
    devices: Vec<FakeDevice>,
    wraps: BTreeMap<String, FakeWrap>,
    /// 测试用：把发给请求设备的封装换成这个（封装、批准者）。
    forged_wrap: Option<(String, String)>,
    log: Vec<String>,
}

fn error(status: u16, code: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: json!({ "error": { "code": code, "message": code } }).to_string(),
    }
}

fn ok(body: Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: body.to_string(),
    }
}

fn no_content() -> HttpResponse {
    HttpResponse {
        status: 204,
        body: String::new(),
    }
}

impl FakeServer {
    fn caller(&self, session: &str) -> Option<String> {
        self.devices
            .iter()
            .find(|device| device.session == session)
            .map(|device| device.device_id.clone())
    }

    fn approved(&self, device: &str) -> bool {
        self.wraps
            .get(device)
            .is_some_and(|wrap| Some(&wrap.key_id) == self.key_id.as_ref())
    }

    fn remove_device(&mut self, device: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|existing| existing.device_id != device);
        self.wraps.remove(device);
        if self.devices.is_empty() {
            self.key_id = None;
        }
        self.devices.len() != before
    }

    fn handle(&mut self, session: &str, request: &HttpRequest) -> HttpResponse {
        self.log.push(format!(
            "{} {}",
            request.method,
            request.path.split('?').next().unwrap()
        ));
        let body: Value = request
            .body
            .as_deref()
            .map(|body| serde_json::from_str(body).unwrap())
            .unwrap_or(Value::Null);
        let caller = self.caller(session);
        let path = request.path.as_str();
        let tail = |prefix: &str| path.strip_prefix(prefix).map(str::to_owned);
        match request.method.as_str() {
            "GET" if path.starts_with("/v1/users/me/learning?device_id=") => {
                let device = &path["/v1/users/me/learning?device_id=".len()..];
                let wrap = self.forged_wrap.clone().or_else(|| {
                    self.wraps
                        .get(device)
                        .filter(|wrap| Some(&wrap.key_id) == self.key_id.as_ref())
                        .map(|wrap| (wrap.wrap.clone(), wrap.approver.clone()))
                });
                let devices = self
                    .devices
                    .iter()
                    .map(|device| {
                        json!({
                            "device_id": device.device_id,
                            "public_key": device.public_key,
                            "name": device.name,
                            "created_at": "2026-11-03T00:00:00Z",
                            "last_seen": "2026-11-03T00:00:00Z",
                            "approved": self.approved(&device.device_id),
                        })
                    })
                    .collect::<Vec<_>>();
                ok(json!({
                    "key_id": self.key_id,
                    "wrap": wrap.map(|(wrap, approver)| json!({ "wrap": wrap, "approver_device_id": approver })),
                    "devices": devices,
                    "blobs": [],
                }))
            }
            "PUT" if path.starts_with("/v1/users/me/learning/devices/") => {
                let device = tail("/v1/users/me/learning/devices/").unwrap();
                let public_key = body["public_key"].as_str().unwrap().to_owned();
                let decoded: [u8; 32] = B64.decode(&public_key).unwrap().try_into().unwrap();
                if device_id(&decoded) != device {
                    return error(400, "invalid_learning_device");
                }
                if let Some(existing) = self.devices.iter_mut().find(|d| d.device_id == device) {
                    if existing.session != session {
                        return error(403, "learning_device_mismatch");
                    }
                    existing.name = body["name"].as_str().unwrap().to_owned();
                    return no_content();
                }
                if let Some(previous) = caller {
                    self.remove_device(&previous);
                }
                self.devices.push(FakeDevice {
                    device_id: device,
                    public_key,
                    name: body["name"].as_str().unwrap().to_owned(),
                    session: session.to_owned(),
                });
                no_content()
            }
            "DELETE" if path.starts_with("/v1/users/me/learning/devices/") => {
                let device = tail("/v1/users/me/learning/devices/").unwrap();
                if self.remove_device(&device) {
                    no_content()
                } else {
                    error(404, "not_found")
                }
            }
            "POST" if path == "/v1/users/me/learning/key" => {
                let Some(caller) = caller else {
                    return error(400, "invalid_learning_key");
                };
                if self.key_id.is_some() {
                    return error(409, "learning_key_conflict");
                }
                let key_id = body["key_id"].as_str().unwrap().to_owned();
                self.key_id = Some(key_id.clone());
                self.wraps.insert(
                    caller.clone(),
                    FakeWrap {
                        key_id,
                        wrap: body["wrap"].as_str().unwrap().to_owned(),
                        approver: caller,
                    },
                );
                no_content()
            }
            "PUT" if path.starts_with("/v1/users/me/learning/wraps/") => {
                let device = tail("/v1/users/me/learning/wraps/").unwrap();
                let approver = body["approver_device_id"].as_str().unwrap().to_owned();
                if caller.as_deref() != Some(approver.as_str()) {
                    return error(403, "learning_device_mismatch");
                }
                let key_id = body["key_id"].as_str().unwrap().to_owned();
                if self.key_id.as_ref() != Some(&key_id) || !self.approved(&approver) {
                    return error(409, "learning_key_conflict");
                }
                if !self.devices.iter().any(|d| d.device_id == device) {
                    return error(400, "invalid_learning_key");
                }
                self.wraps.insert(
                    device,
                    FakeWrap {
                        key_id,
                        wrap: body["wrap"].as_str().unwrap().to_owned(),
                        approver,
                    },
                );
                no_content()
            }
            "POST" if path == "/v1/users/me/learning/rekey" => {
                let revoke = body["revoke"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let Some(caller) = caller.filter(|caller| !revoke.contains(caller)) else {
                    return error(400, "invalid_learning_key");
                };
                if self.key_id.as_deref() != body["previous_key_id"].as_str() {
                    return error(409, "learning_key_conflict");
                }
                for device in &revoke {
                    self.devices
                        .retain(|existing| &existing.device_id != device);
                }
                self.wraps.clear();
                let key_id = body["key_id"].as_str().unwrap().to_owned();
                self.key_id = Some(key_id.clone());
                self.wraps.insert(
                    caller.clone(),
                    FakeWrap {
                        key_id,
                        wrap: body["wrap"].as_str().unwrap().to_owned(),
                        approver: caller,
                    },
                );
                no_content()
            }
            _ => error(404, "not_found"),
        }
    }
}

struct Device {
    name: &'static str,
    session: String,
    keystore: Option<String>,
}

impl Device {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            session: format!("session-{name}"),
            keystore: None,
        }
    }

    fn run(&mut self, server: &mut FakeServer, operation: Operation) -> Result<Value, DriverError> {
        let session = self.session.clone();
        let mut saved = self.keystore.clone();
        let result = run(
            operation,
            self.keystore.as_deref(),
            self.name,
            |request| Ok::<_, ()>(server.handle(&session, request)),
            |keystore| {
                saved = Some(keystore.to_owned());
                Ok(Persisted::Saved)
            },
            || (),
        )
        .unwrap();
        self.keystore = saved;
        result
    }

    fn status(&mut self, server: &mut FakeServer) -> Value {
        self.run(server, Operation::Status).unwrap()
    }

    fn keys(&self) -> Keystore {
        Keystore::parse(self.keystore.as_deref().unwrap()).unwrap()
    }

    fn id(&self) -> String {
        self.keys().device_id()
    }
}

/// 第一台设备建钥，第二台被它批准并核对反向核对码。
fn paired(server: &mut FakeServer) -> (Device, Device) {
    let mut mac = Device::new("MacBook");
    assert_eq!(mac.status(server)["status"], "ready");
    let mut phone = Device::new("Phone");
    assert_eq!(phone.status(server)["status"], "pending");
    let approved = mac
        .run(
            server,
            Operation::Approve {
                device_id: phone.id(),
            },
        )
        .unwrap();
    let shown_on_mac = approved["approved"]["code"].as_str().unwrap().to_owned();
    let status = phone.status(server);
    assert_eq!(status["status"], "confirm");
    assert_eq!(status["confirm"]["code"], shown_on_mac);
    assert_eq!(
        phone.run(server, Operation::Confirm).unwrap()["status"],
        "ready"
    );
    (mac, phone)
}

// ---- 驱动 ----

#[test]
fn the_first_device_creates_the_key_and_is_ready_at_once() {
    let mut server = FakeServer::default();
    let mut mac = Device::new("MacBook");
    let status = mac.status(&mut server);
    assert_eq!(status["status"], "ready");
    assert_eq!(status["notice"], Value::Null);
    assert_eq!(status["device_id"], mac.id());
    assert_eq!(status["code"], mac.keys().verification_code());
    assert_eq!(status["devices"][0]["current"], true);
    assert_eq!(status["devices"][0]["approved"], true);
    assert_eq!(
        server.log,
        [
            "GET /v1/users/me/learning".to_owned(),
            "PUT /v1/users/me/learning/devices/".to_owned() + &mac.id(),
            "GET /v1/users/me/learning".to_owned(),
            "POST /v1/users/me/learning/key".to_owned(),
            "GET /v1/users/me/learning".to_owned(),
        ]
    );
    assert_eq!(mac.keys().ready_key_id(), server.key_id.as_deref());
    // 第一台设备的封装是带认证地封装给自己的。
    assert_eq!(server.wraps[&mac.id()].approver, mac.id());
}

#[test]
fn a_new_device_reads_blobs_only_after_approval_and_confirmation() {
    let mut server = FakeServer::default();
    let mut mac = Device::new("MacBook");
    mac.status(&mut server);
    let mut phone = Device::new("Phone");
    let pending = phone.status(&mut server);
    assert_eq!(pending["status"], "pending");
    assert_eq!(pending["code"], phone.keys().verification_code());
    // 批准方看到的核对码与新设备自己显示的相同。
    let listed = mac.status(&mut server)["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|device| device["device_id"] == phone.id())
        .unwrap()
        .clone();
    assert_eq!(listed["code"], pending["code"]);
    assert_eq!(listed["approved"], false);

    let envelope = mac.keys().seal_blob(1, STREAM.as_bytes()).unwrap();
    assert!(phone.keys().open_blob(&mac.id(), &envelope).is_none());

    let approved = mac
        .run(
            &mut server,
            Operation::Approve {
                device_id: phone.id(),
            },
        )
        .unwrap();
    assert_eq!(approved["approved"]["device_id"], phone.id());
    assert_eq!(approved["approved"]["name"], "Phone");
    let confirm = phone.status(&mut server);
    assert_eq!(confirm["status"], "confirm");
    assert_eq!(confirm["confirm"]["code"], approved["approved"]["code"]);
    assert_eq!(confirm["confirm"]["approver_device_id"], mac.id());
    assert_eq!(confirm["confirm"]["approver_name"], "MacBook");
    // 核对之前不能解密，再取一次状态也还是等核对。
    assert!(phone.keys().open_blob(&mac.id(), &envelope).is_none());
    assert_eq!(phone.status(&mut server)["status"], "confirm");

    assert_eq!(
        phone.run(&mut server, Operation::Confirm).unwrap()["status"],
        "ready"
    );
    assert_eq!(
        phone.keys().open_blob(&mac.id(), &envelope).unwrap().stream,
        STREAM.as_bytes()
    );
    let back = phone.keys().seal_blob(1, b"phone").unwrap();
    assert_eq!(
        mac.keys().open_blob(&phone.id(), &back).unwrap().stream,
        b"phone"
    );
}

#[test]
fn denying_the_reverse_code_drops_the_key_until_someone_else_approves() {
    let mut server = FakeServer::default();
    let (mut mac, _phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.status(&mut server);
    mac.run(
        &mut server,
        Operation::Approve {
            device_id: tablet.id(),
        },
    )
    .unwrap();
    assert_eq!(tablet.status(&mut server)["status"], "confirm");

    let denied = tablet.run(&mut server, Operation::Deny).unwrap();
    assert_eq!(denied["status"], "pending");
    assert_eq!(denied["notice"], "learning_sas_mismatch");
    assert!(tablet.keys().key.as_ref().unwrap().key.is_none());
    // 同一个批准者给同一把密钥的封装不再解开，也不再弹核对。
    let again = tablet.status(&mut server);
    assert_eq!(again["status"], "pending");
    assert_eq!(again["notice"], "learning_sas_mismatch");
    assert!(tablet.keys().ready_key_id().is_none());
    // 拒绝之后再点确认没有可确认的。
    assert_eq!(
        tablet
            .run(&mut server, Operation::Confirm)
            .unwrap_err()
            .code,
        "invalid_state"
    );
}

#[test]
fn a_different_approver_after_a_denial_is_asked_again() {
    let mut server = FakeServer::default();
    let (mut mac, mut phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.status(&mut server);
    mac.run(
        &mut server,
        Operation::Approve {
            device_id: tablet.id(),
        },
    )
    .unwrap();
    tablet.status(&mut server);
    tablet.run(&mut server, Operation::Deny).unwrap();
    let approved = phone
        .run(
            &mut server,
            Operation::Approve {
                device_id: tablet.id(),
            },
        )
        .unwrap();
    let status = tablet.status(&mut server);
    assert_eq!(status["status"], "confirm");
    assert_eq!(status["confirm"]["approver_device_id"], phone.id());
    assert_eq!(status["confirm"]["code"], approved["approved"]["code"]);
}

#[test]
fn a_server_injected_key_shows_a_code_no_real_device_has() {
    let mut server = FakeServer::default();
    let (mut mac, _phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.status(&mut server);
    // 服务端自己登记一台假设备 F，造一把密钥 K' 封装给新设备，并把账号密钥换成 K'。
    let fake = secret(9);
    let fake_public = public(&fake);
    server.devices.push(FakeDevice {
        device_id: device_id(&fake_public),
        public_key: B64.encode(fake_public),
        name: "MacBook".to_owned(),
        session: "server".to_owned(),
    });
    let forged = LearningKey::new([3u8; 32]);
    let tablet_public: [u8; 32] = tablet.keys().public_key();
    let wrap = wrap_key(&forged, &fake, &tablet_public).unwrap().unwrap();
    server.key_id = Some(forged.id.clone());
    server.forged_wrap = Some((wrap, device_id(&fake_public)));

    let status = tablet.status(&mut server);
    assert_eq!(status["status"], "confirm");
    // 真设备上能看到的号码只有它用 K 算的；K' 下的号码与任何一个都不同。
    let genuine = reverse_code(
        &fixed_key_of(&mac),
        &mac.keys().public_key(),
        &tablet_public,
    );
    assert_ne!(status["confirm"]["code"], genuine);
    assert!(tablet.keys().ready_key_id().is_none());
    // 冒用真设备的编号当批准者：解不开，仍在等待。
    server.forged_wrap = Some((
        wrap_key(&forged, &fake, &tablet_public).unwrap().unwrap(),
        mac.id(),
    ));
    tablet.keystore = Some(
        Keystore {
            secret: tablet.keys().secret,
            key: None,
        }
        .encode(),
    );
    assert_eq!(tablet.status(&mut server)["status"], "pending");
    // 真设备在 K' 下已经没有封装：等待重新批准，并提示密钥换过。
    server.forged_wrap = None;
    let mac_status = mac.status(&mut server);
    assert_eq!(mac_status["status"], "pending");
    assert_eq!(mac_status["notice"], "learning_key_rotated");
}

fn fixed_key_of(device: &Device) -> LearningKey {
    device.keys().ready_key().unwrap().clone()
}

#[test]
fn removing_a_device_rotates_the_key_and_others_are_asked_again() {
    let mut server = FakeServer::default();
    let (mut mac, mut phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.status(&mut server);
    mac.run(
        &mut server,
        Operation::Approve {
            device_id: tablet.id(),
        },
    )
    .unwrap();
    tablet.status(&mut server);
    tablet.run(&mut server, Operation::Confirm).unwrap();
    let old_key = mac.keys().ready_key_id().unwrap().to_owned();
    let old_envelope = mac.keys().seal_blob(1, b"old").unwrap();

    let status = mac
        .run(
            &mut server,
            Operation::Remove {
                device_id: phone.id(),
            },
        )
        .unwrap();
    assert_eq!(status["status"], "ready");
    let new_key = mac.keys().ready_key_id().unwrap().to_owned();
    assert_ne!(new_key, old_key);
    assert!(server
        .log
        .contains(&"POST /v1/users/me/learning/rekey".to_owned()));
    assert!(!server
        .devices
        .iter()
        .any(|device| device.device_id == phone.id()));
    // 新密钥解不开旧密钥下的信封。
    assert!(mac.keys().open_blob(&mac.id(), &old_envelope).is_none());

    // 留下的设备要重新批准，并提示换过密钥。
    let tablet_status = tablet.status(&mut server);
    assert_eq!(tablet_status["status"], "pending");
    assert_eq!(tablet_status["notice"], "learning_key_rotated");
    // 被移除的设备重新登记后只是一台待批准的设备。
    let phone_status = phone.status(&mut server);
    assert_eq!(phone_status["status"], "pending");
    assert!(phone.keys().ready_key_id().is_none());
}

#[test]
fn reset_from_a_pending_device_takes_over() {
    let mut server = FakeServer::default();
    let (mut mac, _phone) = paired(&mut server);
    let mut fresh = Device::new("New");
    assert_eq!(fresh.status(&mut server)["status"], "pending");
    let status = fresh.run(&mut server, Operation::Reset).unwrap();
    assert_eq!(status["status"], "ready");
    assert_eq!(fresh.keys().ready_key_id(), server.key_id.as_deref());
    assert_eq!(mac.status(&mut server)["notice"], "learning_key_rotated");
}

#[test]
fn reset_without_any_key_creates_the_first_one() {
    let mut server = FakeServer::default();
    let mut mac = Device::new("MacBook");
    let status = mac.run(&mut server, Operation::Reset).unwrap();
    assert_eq!(status["status"], "ready");
    assert!(server
        .log
        .contains(&"POST /v1/users/me/learning/key".to_owned()));
    assert!(!server
        .log
        .contains(&"POST /v1/users/me/learning/rekey".to_owned()));
}

#[test]
fn concurrent_first_devices_settle_on_one_key() {
    let mut server = FakeServer::default();
    let mut mac = Device::new("MacBook");
    let mut phone = Device::new("Phone");
    // 两台都已登记、都看到没有密钥。
    let mac_session = mac.session.clone();
    let phone_session = phone.session.clone();
    let mut mac_step = start(Operation::Status, None, "MacBook").unwrap();
    let mut phone_step = start(Operation::Status, None, "Phone").unwrap();
    let advance = |server: &mut FakeServer, session: &str, step: Step| match step {
        Step::Request { request, state, .. } => {
            resume(state, server.handle(session, &request)).unwrap()
        }
        other => other,
    };
    // GET → PUT → GET，下一步都是 POST /key。
    for _ in 0..3 {
        mac_step = advance(&mut server, &mac_session, mac_step);
        phone_step = advance(&mut server, &phone_session, phone_step);
    }
    for step in [&mac_step, &phone_step] {
        let Step::Request { request, .. } = step else {
            panic!("expected a request");
        };
        assert_eq!(request.path, "/v1/users/me/learning/key");
    }
    let finish = |server: &mut FakeServer, session: &str, mut step: Step| loop {
        step = match step {
            Step::Done { result, keystore } => break (result, keystore),
            Step::Failed { error, .. } => panic!("{error:?}"),
            step => advance(server, session, step),
        };
    };
    let (mac_result, mac_keystore) = finish(&mut server, &mac_session, mac_step);
    let (phone_result, phone_keystore) = finish(&mut server, &phone_session, phone_step);
    assert_eq!(mac_result["status"], "ready");
    assert_eq!(phone_result["status"], "pending");
    mac.keystore = Some(mac_keystore);
    phone.keystore = Some(phone_keystore);
    assert_eq!(mac.keys().ready_key_id(), server.key_id.as_deref());
}

#[test]
fn approving_requires_a_confirmed_key() {
    let mut server = FakeServer::default();
    let (mut mac, _phone) = paired(&mut server);
    let mut tablet = Device::new("Tablet");
    tablet.status(&mut server);
    let mut other = Device::new("Other");
    other.status(&mut server);
    assert_eq!(
        tablet
            .run(
                &mut server,
                Operation::Approve {
                    device_id: other.id()
                }
            )
            .unwrap_err()
            .code,
        "learning_waiting_approval"
    );
    mac.run(
        &mut server,
        Operation::Approve {
            device_id: tablet.id(),
        },
    )
    .unwrap();
    // 解开了但还没核对，也不能批准别人。
    assert_eq!(
        tablet
            .run(
                &mut server,
                Operation::Approve {
                    device_id: other.id()
                }
            )
            .unwrap_err()
            .code,
        "learning_waiting_approval"
    );
    assert_eq!(
        mac.run(
            &mut server,
            Operation::Approve {
                device_id: "0".repeat(32)
            }
        )
        .unwrap_err()
        .code,
        "not_found"
    );
}

#[test]
fn reject_deletes_a_pending_registration() {
    let mut server = FakeServer::default();
    let (mut mac, _phone) = paired(&mut server);
    let mut stranger = Device::new("Stranger");
    stranger.status(&mut server);
    let status = mac
        .run(
            &mut server,
            Operation::Reject {
                device_id: stranger.id(),
            },
        )
        .unwrap();
    assert!(!status["devices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|device| device["device_id"] == stranger.id()));
    assert_eq!(status["status"], "ready");
}

#[test]
fn disabling_unregisters_and_forgets_the_key_locally() {
    let mut server = FakeServer::default();
    let (_mac, mut phone) = paired(&mut server);
    let phone_id = phone.id();
    let result = phone.run(&mut server, Operation::Disable).unwrap();
    assert_eq!(result["status"], "disabled");
    assert!(phone.keys().key.is_none());
    assert_eq!(phone.id(), phone_id, "the device identity is kept");
    assert!(!server
        .devices
        .iter()
        .any(|device| device.device_id == phone_id));
    // 已经不在服务端也算关好了。
    assert_eq!(
        phone.run(&mut server, Operation::Disable).unwrap()["status"],
        "disabled"
    );
}

#[test]
fn the_last_device_disabling_removes_the_account_key() {
    let mut server = FakeServer::default();
    let mut mac = Device::new("MacBook");
    mac.status(&mut server);
    mac.run(&mut server, Operation::Disable).unwrap();
    assert!(server.key_id.is_none());
    // 再打开时重新建钥。
    assert_eq!(mac.status(&mut server)["status"], "ready");
}

#[test]
fn local_choices_without_a_pending_key_fail_before_any_request() {
    for operation in [Operation::Confirm, Operation::Deny] {
        let step = start(operation, None, "Mac").unwrap();
        let Step::Failed { error, .. } = step else {
            panic!("expected a local failure");
        };
        assert_eq!(error.code, "invalid_state");
    }
}

#[test]
fn invalid_operations_fail_before_any_request() {
    let keystore = Keystore::generate().unwrap();
    let own = keystore.device_id();
    let encoded = keystore.encode();
    for operation in [
        Operation::Approve {
            device_id: own.clone(),
        },
        Operation::Reject {
            device_id: "XYZ".to_owned(),
        },
        Operation::Remove {
            device_id: "A".repeat(32),
        },
    ] {
        let step = start(operation, Some(&encoded), "Mac").unwrap();
        let Step::Failed { error, .. } = step else {
            panic!("expected a local failure");
        };
        assert_eq!(error.code, "invalid_device_id");
    }
}

#[test]
fn server_errors_surface_their_code() {
    let result = run(
        Operation::Status,
        None,
        "MacBook",
        |_| Ok::<_, ()>(error(403, "learning_requires_account")),
        |_| Ok(Persisted::Saved),
        || (),
    )
    .unwrap();
    assert_eq!(
        result.unwrap_err(),
        DriverError {
            status: 403,
            code: "learning_requires_account".to_owned()
        }
    );
}

#[test]
fn a_lying_key_conflict_loop_gives_up() {
    // 服务端每次都说没有密钥、建钥又总是 409：重取状态有上限。
    let keystore = Keystore::generate().unwrap();
    let own_public = keystore.public_key();
    let state = json!({
        "key_id": null,
        "wrap": null,
        "devices": [{ "device_id": device_id(&own_public), "public_key": B64.encode(own_public) }],
    });
    let result = run(
        Operation::Status,
        Some(&keystore.encode()),
        "Mac",
        |request| {
            Ok::<_, ()>(if request.method == "GET" {
                ok(state.clone())
            } else {
                error(409, "learning_key_conflict")
            })
        },
        |_| Ok(Persisted::Saved),
        || (),
    )
    .unwrap();
    assert_eq!(result.unwrap_err().code, "learning_key_conflict");
}

#[test]
fn run_adopts_the_identity_another_process_saved_first() {
    let mut server = FakeServer::default();
    let stored = Keystore::generate().unwrap().encode();
    let mut persisted = Vec::new();
    let result = run(
        Operation::Status,
        None,
        "Mac",
        |request| Ok::<_, ()>(server.handle("s", request)),
        |keystore| {
            persisted.push(keystore.to_owned());
            Ok(if persisted.len() == 1 {
                Persisted::Conflict(stored.clone())
            } else {
                Persisted::Saved
            })
        },
        || (),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        result["device_id"],
        Keystore::parse(&stored).unwrap().device_id()
    );
    assert_eq!(server.devices.len(), 1);
}

#[test]
fn the_json_entry_point_dispatches_by_domain() {
    let learning = Keystore::generate().unwrap();
    let clipboard = super::super::clipboard_e2e::Keystore::generate().unwrap();
    let start_with = |domain: Option<&str>, keystore: &str| {
        let mut input =
            json!({ "start": { "op": "status" }, "keystore": keystore, "device_name": "Mac" });
        if let Some(domain) = domain {
            input["domain"] = json!(domain);
        }
        serde_json::to_vec(&input).unwrap()
    };

    // 不带 domain 时与直接调云剪贴板驱动完全相同。
    let plain = start_with(None, &clipboard.encode());
    assert_eq!(
        crate::cloud::e2e_drive_json(&plain).unwrap(),
        super::super::clipboard_e2e::drive_json(&plain).unwrap()
    );
    assert_eq!(
        crate::cloud::e2e_drive_json(&start_with(Some("clipboard"), &clipboard.encode())).unwrap(),
        super::super::clipboard_e2e::drive_json(&plain).unwrap()
    );
    assert_eq!(
        crate::cloud::e2e_drive_json(b"not json").unwrap_err(),
        super::super::clipboard_e2e::drive_json(b"not json").unwrap_err()
    );

    let step =
        crate::cloud::e2e_drive_json(&start_with(Some("learning"), &learning.encode())).unwrap();
    assert_eq!(step["request"]["method"], "GET");
    assert_eq!(
        step["request"]["path"],
        format!("/v1/users/me/learning?device_id={}", learning.device_id())
    );
    assert_eq!(step["keystore"], learning.encode());

    // 交回响应时也要带 domain。
    let resumed = json!({
        "domain": "learning",
        "resume": step["state"],
        "response": { "status": 403, "body": "{\"error\":{\"code\":\"learning_requires_account\"}}" },
    });
    let failed = crate::cloud::e2e_drive_json(&serde_json::to_vec(&resumed).unwrap()).unwrap();
    assert_eq!(failed["error"]["code"], "learning_requires_account");

    assert!(crate::cloud::e2e_drive_json(&start_with(Some("dictionary"), "")).is_err());
    let mut unknown = json!({ "domain": "learning", "start": { "op": "status" }, "extra": 1 });
    assert!(crate::cloud::e2e_drive_json(&serde_json::to_vec(&unknown).unwrap()).is_err());
    unknown["start"] = json!({ "op": "list" });
    assert!(crate::cloud::e2e_drive_json(&serde_json::to_vec(&unknown).unwrap()).is_err());
}
