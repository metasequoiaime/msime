use super::*;

fn animated_gif(count: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for i in 0..count {
            let image = image::RgbaImage::from_pixel(4, 3, image::Rgba([i as u8, 100, 200, 180]));
            encoder
                .encode_frame(image::Frame::from_parts(
                    image,
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(70, 1),
                ))
                .unwrap();
        }
    }
    bytes
}

// 用 Pillow 对 4×3 纯色合成帧生成，不依赖外部素材。
fn animated_webp() -> Vec<u8> {
    BASE64.decode("UklGRogAAABXRUJQVlA4WAoAAAASAAAAAwAAAgAAQU5JTQYAAAAAAAAAAwBBTk1GKgAAAAAAAAAAAAMAAAIAAEYAAAJWUDhMEQAAAC8DgAAAB1DBalW6/4GI6H8AAEFOTUYqAAAAAAAAAAAAAwAAAgAAlgAAAlZQOEwRAAAALwOAABAHUJRy16hkgYjofwAA").unwrap()
}

fn canvas_fixture(root: &Path) -> PackedSkin {
    let skin = root.join("animated-canvas");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("skin.toml"), "schema_version = 2\nid = 'animated-canvas'\nname = '合成画布'\nversion = '1'\nbase = 'canvas'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['dark']\n[candidate_window.border_image]\nsource = 'panel.png'\nslice_dip = [1]\n[[candidate_window.decorations]]\nimage = 'petals.apng'\nwidth_dip = 10\nheight_dip = 10\n[[candidate_window.decorations]]\nimage = 'stars.gif'\nwidth_dip = 10\nheight_dip = 10\n[[candidate_window.decorations]]\nimage = 'moon.webp'\nwidth_dip = 4\nheight_dip = 3\n[license]\nassets = 'CC0'\n").unwrap();
    for (path, bytes) in [
        ("preview.png", png(1000)),
        ("panel.png", png(1000)),
        ("petals.apng", apng(4, 3, 3)),
        ("stars.gif", animated_gif(3)),
        ("moon.webp", animated_webp()),
    ] {
        fs::write(skin.join(path), bytes).unwrap();
    }
    pack(root, "animated-canvas").unwrap()
}

#[test]
fn canvas_images_round_trip_through_packing_and_atomic_install() {
    let source = tempfile::tempdir().unwrap();
    let packed = canvas_fixture(source.path());
    assert_eq!(packed.file_count, 5);
    let downloaded = CandidateSkinPackage {
        id: item().id,
        package_id: packed.package_id,
        manifest: packed.manifest,
        files: packed.files,
    };
    let target = tempfile::tempdir().unwrap();
    assert_eq!(
        install(target.path(), &downloaded, false).unwrap(),
        "animated-canvas"
    );
    let reloaded = catalog::load_package(target.path(), "animated-canvas").unwrap();
    assert!(reloaded.canvas);
    assert_eq!(reloaded.decorations.len(), 3);
    for (path, encoded) in &downloaded.files {
        assert_eq!(
            fs::read(target.path().join("animated-canvas").join(path)).unwrap(),
            BASE64.decode(encoded).unwrap()
        );
    }
    assert_eq!(
        pack(target.path(), "animated-canvas").unwrap().files,
        downloaded.files
    );
    assert!(!target.path().join(STAGING_DIRECTORY).exists());
}

#[test]
fn image_toolbar_resources_round_trip_at_the_full_local_file_limit() {
    let source = tempfile::tempdir().unwrap();
    let skin = source.path().join("full-toolbar");
    fs::create_dir(&skin).unwrap();
    let mut manifest = String::from("schema_version=2\nid='full-toolbar'\nname='合成工具栏'\nversion='1'\nbase='canvas'\npreview='preview.png'\n[supports]\nlayouts=['horizontal']\nthemes=['light']\n[candidate_window.decoration]\ntop_inset_dip=4\nwidth_dip=4\nimage='decoration.png'\n[candidate_window.background]\nimage='background.png'\n[candidate_window.border_image]\nsource='panel.png'\nlight_source='panel-light.png'\nslice_dip=[1]\n");
    for index in 0..32 {
        manifest.push_str(&format!("[[candidate_window.decorations]]\nimage='widget-{index}.png'\nwidth_dip=4\nheight_dip=4\n"));
    }
    manifest.push_str("[toolbar]\nmode='image'\nimage='toolbar.png'\nimage_width_dip=168\nimage_height_dip=48\n[toolbar.button_defaults]\nbackground='default.png'\n[toolbar.button_flow]\n");
    for (key, states) in [
        ("status", &["cn", "en", "ja", "caps", "en-candidate"][..]),
        ("fullwidth", &["full", "half"][..]),
        ("punctuation", &["cn", "en"][..]),
        ("character_set", &["simplified", "traditional"][..]),
        ("emoji", &[][..]),
        ("screen_keyboard", &[][..]),
        ("settings", &[][..]),
    ] {
        manifest.push_str(&format!(
            "[toolbar.buttons.{key}]\nimage='{key}.png'\nbackground='{key}-bg.png'\n"
        ));
        if !states.is_empty() {
            manifest.push_str(&format!("[toolbar.buttons.{key}.images]\n"));
            for state in states {
                manifest.push_str(&format!("{state}='{key}-{state}.png'\n"));
            }
        }
    }
    manifest.push_str("[license]\nassets='CC0'\n");
    fs::write(skin.join("skin.toml"), &manifest).unwrap();
    // 从合成清单枚举路径，所有 64 张独立资源都必须随安装保留。
    for line in manifest.lines() {
        if let Some((_, value)) = line.split_once('=') {
            let path = value.trim_matches('\'');
            if path.ends_with(".png") {
                fs::write(skin.join(path), png(0)).unwrap();
            }
        }
    }
    let summary = catalog::load_package(source.path(), "full-toolbar").unwrap();
    assert_eq!(catalog::referenced_image_paths(&summary).len(), 64);
    let packed = pack(source.path(), "full-toolbar").unwrap();
    assert_eq!(packed.file_count, MAX_LOCAL_PACKAGE_FILES);
    assert!(packed.file_count > MAX_PACKAGE_FILES);
    let package = CandidateSkinPackage {
        id: item().id,
        package_id: packed.package_id,
        manifest: packed.manifest,
        files: packed.files,
    };
    let target = tempfile::tempdir().unwrap();
    install(target.path(), &package, false).unwrap();
    assert_eq!(
        pack(target.path(), "full-toolbar").unwrap().files,
        package.files
    );
    let reloaded = catalog::load_package(target.path(), "full-toolbar").unwrap();
    assert_eq!(reloaded.toolbar_image.as_ref().unwrap().buttons.len(), 7);
    // 社区协议上限保持独立，不能把本地扩容默默当作服务器也接受。
    assert_eq!(validate_package(&package), Err(AccountError::Unavailable));
    let mut too_many = package.clone();
    too_many
        .files
        .insert("extra.png".into(), BASE64.encode(png(0)));
    let before = snapshot(target.path());
    assert_eq!(install(target.path(), &too_many, true), Err(FILE_PATH));
    assert_eq!(snapshot(target.path()), before);
    // 后续状态图损坏时整包原子替换失败，既有皮肤不变。
    too_many.files.remove("extra.png");
    too_many
        .files
        .insert("status-en.png".into(), BASE64.encode(b"invalid image"));
    assert!(install(target.path(), &too_many, true).is_err());
    assert_eq!(snapshot(target.path()), before);
}

#[test]
fn image_toolbar_upload_needs_its_own_server_capability() {
    let root = tempfile::tempdir().unwrap();
    let skin = write_skin(root.path(), "toolbar-test", TOP, "", "", LICENSE);
    let path = skin.join("skin.toml");
    let manifest = format!("{}\n[toolbar]\nmode='image'\nimage='preview.png'\nimage_width_dip=168\nimage_height_dip=48\n", fs::read_to_string(&path).unwrap());
    fs::write(path, manifest).unwrap();
    let request = CandidateSkinPublishRequest::new(
        item().id,
        "合成工具栏".into(),
        "".into(),
        pack(root.path(), "toolbar-test").unwrap(),
        CandidateSkinVisibility::Public,
        None,
    );
    for header in [
        "",
        "X-MSIME-Candidate-Resources: canvas-v1\r\n",
        "X-MSIME-Candidate-Toolbar: unknown\r\n",
    ] {
        let (origin, received) = serve_responses(vec![(200, header, b"{}".to_vec())]);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client.publish_candidate_skin(&request, &token(b'x')),
            Err(AccountError::Invalid)
        );
        let (head, body) = received.recv().unwrap();
        assert!(head.starts_with("GET "));
        assert!(body.is_empty());
        assert!(received.recv().is_err());
    }
    let (origin, received) = serve_responses(vec![
        (
            200,
            "X-MSIME-Candidate-Toolbar: image-v1\r\n",
            b"{}".to_vec(),
        ),
        (200, "", serde_json::to_vec(&item()).unwrap()),
    ]);
    assert!(BackendAccountClient::loopback(&origin)
        .unwrap()
        .publish_candidate_skin(&request, &token(b'x'))
        .is_ok());
    assert!(received.recv().unwrap().0.starts_with("GET "));
    assert!(received.recv().unwrap().0.starts_with("POST "));
}

#[test]
fn animated_resources_are_strictly_decoded_and_bounded() {
    for (kind, bytes, frames) in [
        ("image/gif", animated_gif(3), 3),
        ("image/webp", animated_webp(), 2),
    ] {
        assert_eq!(check_image(kind, &bytes), Ok(12 * frames));
        assert_eq!(
            check_image_with_budget(kind, &bytes, 12 * frames - 1),
            Err(TOO_LARGE)
        );
        for cut in [6, bytes.len() / 2, bytes.len() - 8] {
            assert!(
                check_image(kind, &bytes[..cut]).is_err(),
                "{kind} 截断于 {cut}"
            );
        }
    }
    assert_eq!(
        check_image(
            "image/gif",
            &animated_gif(catalog::MAX_ANIMATION_FRAMES + 1)
        ),
        Err(TOO_LARGE)
    );
    let mut bomb = animated_gif(1);
    bomb[6..8].copy_from_slice(&65535u16.to_le_bytes());
    assert_eq!(check_image("image/gif", &bomb), Err(TOO_LARGE));
}

#[test]
fn webp_preflight_protects_uploads_and_keeps_valid_alpha_frames() {
    use crate::skin::webp_preflight::tests::{animated, static_images, with_inner_dimensions};
    let images = static_images();
    for bytes in &images {
        assert_eq!(check_image("image/webp", bytes), Ok(12));
        let mismatch = with_inner_dimensions(bytes.clone(), 5, 3);
        assert_eq!(
            check_image("image/webp", &animated(&[mismatch])),
            Err(IMAGE_INVALID)
        );
    }
    assert_eq!(check_image("image/webp", &animated(&images)), Ok(36));
}

#[test]
fn corrupt_late_animation_frame_leaves_installed_files_unchanged() {
    let source = tempfile::tempdir().unwrap();
    let packed = canvas_fixture(source.path());
    let mut downloaded = CandidateSkinPackage {
        id: item().id,
        package_id: packed.package_id,
        manifest: packed.manifest,
        files: packed.files,
    };
    let target = tempfile::tempdir().unwrap();
    install(target.path(), &downloaded, false).unwrap();
    let before = snapshot(target.path());
    let mut bytes = animated_webp();
    let length = bytes.len();
    bytes[length - 7..length - 1].fill(0xff);
    use crate::skin::webp_preflight::tests::{animated, static_images, with_inner_dimensions};
    let images = static_images();
    let late_mismatch = animated(&[
        images[0].clone(),
        with_inner_dimensions(images[2].clone(), 5, 3),
    ]);
    for broken in [bytes, late_mismatch] {
        downloaded
            .files
            .insert("moon.webp".into(), BASE64.encode(broken));
        assert!(install(target.path(), &downloaded, true).is_err());
        assert_eq!(snapshot(target.path()), before);
    }
}

#[test]
fn preview_formats_stay_compatible_with_old_clients() {
    for (path, kind, data) in [
        ("preview.gif", "image/gif", animated_gif(2)),
        ("preview.webp", "image/webp", animated_webp()),
        ("preview.apng", "image/png", apng(4, 3, 2)),
    ] {
        assert_eq!(
            validate_preview(&CandidateSkinPreview {
                path: path.into(),
                content_type: kind.into(),
                data: BASE64.encode(data)
            }),
            Err(AccountError::Unavailable)
        );
    }
}

// 有界响应序列让测试能核对探测与写入的顺序，且旧部署案例只接受一次请求。
fn serve_responses(
    responses: Vec<(u16, &'static str, Vec<u8>)>,
) -> (String, mpsc::Receiver<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for (status, headers, response) in responses {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            sent.send((head, body)).unwrap();
            let mut stream = reader.into_inner();
            write!(stream,"HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{headers}\r\n",response.len()).unwrap();
            stream.write_all(&response).unwrap();
        }
    });
    (origin, received)
}

#[test]
fn an_old_or_ambiguous_deployment_never_receives_extended_upload_bytes() {
    let root = tempfile::tempdir().unwrap();
    for header in ["", "X-MSIME-Candidate-Resources: unknown\r\n"] {
        let packed = canvas_fixture(root.path());
        let request = CandidateSkinPublishRequest::new(
            item().id,
            "合成画布".into(),
            "".into(),
            packed,
            CandidateSkinVisibility::Public,
            None,
        );
        let (origin, received) = serve_responses(vec![(200, header, b"{}".to_vec())]);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client.publish_candidate_skin(&request, &token(b'x')),
            Err(AccountError::Invalid)
        );
        let (head, body) = received.recv().unwrap();
        assert!(head.starts_with("GET /v1/community/candidate-skins?resources=canvas-v1 HTTP/1.1"));
        assert!(body.is_empty());
        assert!(received.recv().is_err());
    }
}

#[test]
fn extended_upload_requires_an_explicit_successful_capability_response() {
    let root = tempfile::tempdir().unwrap();
    let request = CandidateSkinPublishRequest::new(
        item().id,
        "合成画布".into(),
        "".into(),
        canvas_fixture(root.path()),
        CandidateSkinVisibility::Public,
        None,
    );
    let response = serde_json::to_vec(&item()).unwrap();
    let (origin, received) = serve_responses(vec![
        (
            200,
            "X-MSIME-Candidate-Resources: canvas-v1\r\n",
            b"{}".to_vec(),
        ),
        (200, "", response),
    ]);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert!(client
        .publish_candidate_skin(&request, &token(b'x'))
        .is_ok());
    assert!(received.recv().unwrap().0.starts_with("GET "));
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with(
        "POST /v1/community/candidate-skins?include=category&resources=canvas-v1 HTTP/1.1"
    ));
    let uploaded: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(uploaded["files"].as_object().unwrap().len(), 5);
    let (origin, received_failure) = serve_responses(vec![(
        503,
        "X-MSIME-Candidate-Resources: canvas-v1\r\n",
        b"{}".to_vec(),
    )]);
    assert_eq!(
        BackendAccountClient::loopback(&origin)
            .unwrap()
            .publish_candidate_skin(&request, &token(b'x')),
        Err(AccountError::Unavailable)
    );
    assert!(received_failure.recv().unwrap().0.starts_with("GET "));
}

#[test]
fn replacement_negotiates_before_sending_extended_resources() {
    let root = tempfile::tempdir().unwrap();
    let packed = canvas_fixture(root.path());
    let request = CandidateSkinReplaceRequest {
        name: "合成画布".into(),
        description: String::new(),
        manifest: packed.manifest,
        files: packed.files,
    };
    for supported in [false, true] {
        let mut responses = vec![(
            200,
            if supported {
                "X-MSIME-Candidate-Resources: canvas-v1\r\n"
            } else {
                ""
            },
            b"{}".to_vec(),
        )];
        if supported {
            responses.push((200, "", serde_json::to_vec(&item()).unwrap()));
        }
        let (origin, received) = serve_responses(responses);
        let result = BackendAccountClient::loopback(&origin)
            .unwrap()
            .replace_candidate_skin(item().id, &request, &token(b'x'));
        assert!(received.recv().unwrap().0.starts_with("GET "));
        if supported {
            assert_eq!(result, Ok(item()));
            let (head, body) = received.recv().unwrap();
            assert!(head.starts_with(&format!(
                "PUT /v1/community/candidate-skins/{}?include=category&resources=canvas-v1 HTTP/1.1",
                item().id.hyphenated()
            )));
            let uploaded: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(uploaded["files"].as_object().unwrap().len(), 5);
        } else {
            assert_eq!(result, Err(AccountError::Invalid));
        }
        assert!(received.recv().is_err());
    }
}
