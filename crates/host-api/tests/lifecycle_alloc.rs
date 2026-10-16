//! 生命周期核心在按键路径上不分配内存：`msime_client_key`、`set_field_traits`、`set_active` 经内置消费者走到播放器队列，一次选词上屏经真实的会话状态走过推导和内置消费者的上屏分支，事件推导本身是纯函数。
//!
//! 计数分配器只数当前测试线程上的分配：播放器自己的线程解码、混音、打开设备时的分配不算，那些本来就不在按键路径上。测量前先预热播放器，第一次按键会起线程、建队列。

// 集成测试各自是独立的 crate，库根上的豁免管不到这里。理由同库根：这个测试直接驱动 C ABI。
#![allow(unsafe_code)]

use msime_client_core::preferences::{DefaultImeMode, Preferences};
use msime_client_services::plugins::EffectStyle;
use msime_host_api::lifecycle::{
    derive, replay, Call, CommitSample, Context, InputClass, PageSample, Sample, FEEDBACK,
    KEY_POINTER, KEY_REPEAT, TRAIT_FULLSCREEN, TRAIT_KNOWN, TRAIT_SECURE,
};
use msime_host_api::{
    msime_client_create, msime_client_destroy, msime_client_key, msime_client_key_sound,
    msime_client_set_active, msime_client_set_field_traits, msime_client_set_lifecycle,
    msime_client_string_free,
};
use msime_input_runtime::{DispatchFacts, SelectionFact, SelectionHow, SelectionOutcome};
use std::ffi::{c_char, CStr};
use std::time::{Duration, Instant};

#[path = "support/allocations.rs"]
mod allocation_counter;
use allocation_counter::allocations;

unsafe fn take_response(pointer: *mut c_char) -> serde_json::Value {
    assert!(!pointer.is_null());
    // SAFETY: 本库返回、归调用方所有、以 NUL 结尾的回复。
    let bytes = unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec();
    unsafe { msime_client_string_free(pointer) };
    serde_json::from_slice(&bytes).unwrap()
}

/// 开着按键音、上屏音、特效和连击的会话，内置的音效包从仓库的 `resources/sound-packs` 读。
fn feedback_session(root: &std::path::Path) -> u64 {
    let path = |name: &str| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let mut preferences = Preferences {
        default_ime_mode: DefaultImeMode::Chinese,
        ..Preferences::default()
    };
    preferences.plugins.key_sound.enabled = true;
    preferences.plugins.commit_sound.enabled = true;
    preferences.plugins.effect_style = EffectStyle::Flash;
    preferences.plugins.combo_counter = true;
    let sound_packs =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/sound-packs");
    let options = serde_json::json!({
        "api_version": 1,
        "resources": path("resources"),
        "user_data": path("user"),
        "cache": path("cache"),
        "dictionaries": path("dictionaries"),
        "sound_packs": sound_packs,
        "preferences": preferences,
    })
    .to_string();
    // SAFETY: 缓冲区活得比这次同步调用长。
    let created = unsafe { take_response(msime_client_create(options.as_ptr(), options.len())) };
    assert_eq!(created["ok"], true, "{created}");
    created["value"]["session"].as_u64().unwrap()
}

#[test]
fn the_key_path_and_event_derivation_allocate_nothing() {
    // 计数器自己得数得到分配，否则下面的 0 什么都不说明。
    assert!(allocations(|| drop(std::hint::black_box(Vec::<u8>::with_capacity(8)))) > 0);
    let root = tempfile::tempdir().unwrap();
    let handle = feedback_session(root.path());
    assert_eq!(msime_client_set_lifecycle(handle, FEEDBACK), FEEDBACK);
    assert_eq!(
        msime_client_set_field_traits(handle, TRAIT_KNOWN),
        TRAIT_KNOWN
    );
    // 预热：第一次按键起播放器线程、建队列，第一次激活下发音乐状态。
    msime_client_key(handle, 0);
    msime_client_set_active(handle, true);

    let keys = allocations(|| {
        for round in 0..200u32 {
            let effect = msime_client_key(handle, round % 4);
            assert_ne!(effect, 0);
            msime_client_key(handle, KEY_REPEAT);
            msime_client_key(handle, KEY_POINTER);
        }
    });
    assert_eq!(keys, 0, "msime_client_key allocated");

    let fields = allocations(|| {
        for _ in 0..50 {
            msime_client_set_field_traits(handle, TRAIT_KNOWN | TRAIT_SECURE);
            assert_eq!(msime_client_key(handle, 0), 0);
            msime_client_set_field_traits(handle, TRAIT_KNOWN | TRAIT_FULLSCREEN);
            msime_client_set_field_traits(handle, TRAIT_KNOWN);
            msime_client_set_active(handle, false);
            msime_client_set_active(handle, true);
        }
    });
    assert_eq!(fields, 0, "set_field_traits or set_active allocated");

    // 一整次选词上屏：selected、before_commit、commit、end、hidden，经真实的会话状态走到内置消费者的上屏分支（上屏音和上屏特效）。先上屏一次预热。
    let composing = Sample {
        composing: true,
        composition_epoch: 3,
        candidates_epoch: 3,
        candidates_visible: true,
    };
    let facts = DispatchFacts {
        selection: Some(SelectionFact {
            seat: 0,
            source: 0,
            how: SelectionHow::PunctuationFinish,
            outcome: SelectionOutcome::Committed,
        }),
        tail_chars: 1,
        ..DispatchFacts::default()
    };
    let commit = Some(CommitSample {
        chars: 3,
        plain: true,
        typed: true,
    });
    let context = Context {
        flags: FEEDBACK,
        traits: TRAIT_KNOWN,
        pieces_held: false,
    };
    let commit_once = || {
        replay(
            handle,
            Call::Dispatch(InputClass::Punctuation),
            composing,
            Sample::default(),
            facts,
            commit,
            PageSample::default(),
        )
    };
    assert!(commit_once().is_some_and(|effect| effect != 0));
    let commits = allocations(|| {
        for _ in 0..100 {
            let effect = commit_once();
            assert!(effect.is_some_and(|effect| effect != 0), "{effect:?}");
        }
    });
    assert_eq!(
        commits, 0,
        "a commit through the built-in consumer allocated"
    );

    let mut emitted = 0;
    let derivation = allocations(|| {
        for _ in 0..100 {
            derive(
                Call::Dispatch(InputClass::Punctuation),
                context,
                composing,
                Sample::default(),
                facts,
                commit,
                PageSample::default(),
                |_| emitted += 1,
            );
            derive(
                Call::Dispatch(InputClass::Character),
                context,
                Sample::default(),
                composing,
                DispatchFacts::default(),
                None,
                PageSample {
                    count: 5,
                    page: 0,
                    highlighted: Some(0),
                },
                |_| emitted += 1,
            );
        }
    });
    assert_eq!(derivation, 0, "derive allocated");
    assert_eq!(emitted, 100 * (5 + 3));

    // 上面量到的按键和上屏确实走到了播放器队列，而不是在播放器关掉后的早退里：播放器一旦关掉就不再打开，所以测量之后它还收声音，测量期间就一直开着。去掉 FEEDBACK 后旧调用不再是空操作，用它的回答来问。上面一口气塞了几百个声音，64 格的队列多半是满的，等播放器线程腾出位置再问。
    assert_eq!(msime_client_set_lifecycle(handle, 0), 0);
    let deadline = Instant::now() + Duration::from_secs(5);
    let alive = loop {
        if msime_client_key_sound(handle, 0) {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !alive {
        eprintln!("播放器已关闭（这台机器没有可用的声音线程）：上面的测量没有走到播放器队列，只覆盖了推导和特效");
    }

    // SAFETY: 本库刚返回的回复。
    let destroyed = unsafe { take_response(msime_client_destroy(handle)) };
    assert_eq!(destroyed["ok"], true);
}
