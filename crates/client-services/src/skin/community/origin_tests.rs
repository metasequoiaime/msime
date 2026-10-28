//! 排除 AI 生成的社区皮肤（`exclude=ai`）：列表和详情带上筛选参数和 `fields=origin`，下载只带筛选参数，只有带着 `X-MSIME-Skin-Origin: v1` 的响应才算筛过，旧部署对 `fields=origin` 答的 400 也按社区不可用处理；来源不是 `user` 的条目再在客户端去掉一次。不排除时路径和以前一样，也不要求这个头。连本机回环上的假服务端，不联网。

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

fn token() -> String {
    "t".repeat(64)
}

fn skin(id: &str, origin: Option<SkinOrigin>) -> CommunitySkin {
    CommunitySkin {
        id: Uuid::parse_str(id).unwrap(),
        name: "合成皮肤".into(),
        description: "仅用于协议测试".into(),
        author: "Fixture".into(),
        design: TouchKeyboardSkinDesign::default(),
        downloads: 0,
        rating_count: 0,
        rating_average: 0.0,
        owned: false,
        my_rating: 0,
        moderation: None,
        category: Some(SkinCategory::Other),
        origin,
    }
}

const USER: &str = "10000000-0000-4000-8000-000000000001";
const AI: &str = "10000000-0000-4000-8000-000000000002";
const UNKNOWN: &str = "10000000-0000-4000-8000-000000000003";
const MISSING: &str = "10000000-0000-4000-8000-000000000004";
const HEADER: &str = "X-MSIME-Skin-Origin: v1\r\n";

/// 依次回答每个连接一个响应（状态行、额外的头、正文），收到的请求行按顺序交回。
fn serve(
    responses: Vec<(&'static str, &'static str, Vec<u8>)>,
) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for (status, headers, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
            let length = stream.read(&mut request).unwrap();
            let text = String::from_utf8_lossy(&request[..length]).into_owned();
            sent.send(text.lines().next().unwrap_or_default().to_owned())
                .unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{headers}\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    (origin, received)
}

fn page(skins: Vec<CommunitySkin>) -> Vec<u8> {
    serde_json::to_vec(&CommunitySkinPage {
        skins,
        has_more: false,
    })
    .unwrap()
}

fn client(origin: &str) -> BackendAccountClient {
    BackendAccountClient::loopback(origin).unwrap()
}

#[test]
fn an_excluding_list_asks_for_the_filter_and_keeps_only_user_items() {
    // 服务端只该交来 `user`；交来 `ai`、`unknown` 或没带 `origin` 的条目都在客户端去掉。
    let (origin, received) = serve(vec![(
        "200 OK",
        HEADER,
        page(vec![
            skin(USER, Some(SkinOrigin::User)),
            skin(AI, Some(SkinOrigin::Ai)),
            skin(UNKNOWN, Some(SkinOrigin::Unknown)),
            skin(MISSING, None),
        ]),
    )]);
    let listed = client(&origin)
        .community_skins(0, "", false, Some(SkinCategory::Acg), true, None)
        .unwrap();
    assert_eq!(listed.skins, vec![skin(USER, Some(SkinOrigin::User))]);
    assert_eq!(
        received.recv().unwrap(),
        "GET /v1/community/skins?offset=0&q=&fields=origin&category=acg&include=category&exclude=ai HTTP/1.1"
    );

    // 作者自己的作品另要审核状态，两项合在同一个 `fields` 里。
    let (origin, received) = serve(vec![("200 OK", HEADER, page(Vec::new()))]);
    client(&origin)
        .community_skins(0, "", true, None, true, Some(&token()))
        .unwrap();
    assert_eq!(
        received.recv().unwrap(),
        "GET /v1/community/skins?offset=0&q=&scope=mine&fields=moderation,origin&include=category&exclude=ai HTTP/1.1"
    );
}

#[test]
fn a_server_that_does_not_declare_the_filter_is_not_trusted() {
    // 旧部署忽略 `exclude` 照常作答；没有能力头时不交出这一页，宁可社区暂不可用。
    for header in ["", "X-MSIME-Skin-Origin: v2\r\n"] {
        let (origin, _received) = serve(vec![("200 OK", header, page(vec![skin(USER, None)]))]);
        assert_eq!(
            client(&origin).community_skins(0, "", false, None, true, None),
            Err(AccountError::Unavailable)
        );
    }
    let (origin, _received) = serve(vec![(
        "200 OK",
        "",
        serde_json::to_vec(&skin(USER, None)).unwrap(),
    )]);
    assert_eq!(
        client(&origin).community_skin(Uuid::parse_str(USER).unwrap(), true, None),
        Err(AccountError::Unavailable)
    );
    let (origin, _received) = serve(vec![(
        "200 OK",
        "",
        serde_json::to_vec(&serde_json::json!({ "design": TouchKeyboardSkinDesign::default() }))
            .unwrap(),
    )]);
    assert_eq!(
        client(&origin).download_community_skin(Uuid::parse_str(USER).unwrap(), true, &token()),
        Err(AccountError::Unavailable)
    );
    // 旧部署的 `fields` 不认 `origin`，列表和详情答 400 `invalid_fields`：同样是社区不可用，不是参数错误。
    let invalid_fields = br#"{"error":{"code":"invalid_fields","message":"invalid_fields"}}"#;
    let (origin, _received) = serve(vec![
        ("400 Bad Request", "", invalid_fields.to_vec()),
        ("400 Bad Request", "", invalid_fields.to_vec()),
    ]);
    let client = client(&origin);
    assert_eq!(
        client.community_skins(0, "", false, None, true, None),
        Err(AccountError::Unavailable)
    );
    assert_eq!(
        client.community_skin(Uuid::parse_str(USER).unwrap(), true, None),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn a_bad_request_without_exclusion_is_still_invalid() {
    // 只有排除请求把 400 读成不可用；其他宿主的请求照旧报参数错误。
    let (origin, _received) = serve(vec![(
        "400 Bad Request",
        "",
        br#"{"error":{"code":"invalid_fields","message":"invalid_fields"}}"#.to_vec(),
    )]);
    assert_eq!(
        client(&origin).community_skins(0, "", false, None, false, None),
        Err(AccountError::Invalid)
    );
}

#[test]
fn an_excluded_detail_or_download_is_not_found() {
    let id = Uuid::parse_str(AI).unwrap();
    // 服务端对被排除的作品答 404；即使它交来了，标着 AI 生成的详情也按不存在处理。
    let (origin, received) = serve(vec![
        (
            "200 OK",
            HEADER,
            serde_json::to_vec(&skin(AI, Some(SkinOrigin::Ai))).unwrap(),
        ),
        (
            "404 Not Found",
            HEADER,
            br#"{"error":{"code":"not_found","message":"not_found"}}"#.to_vec(),
        ),
        (
            "404 Not Found",
            HEADER,
            br#"{"error":{"code":"not_found","message":"not_found"}}"#.to_vec(),
        ),
    ]);
    let client = client(&origin);
    assert_eq!(
        client.community_skin(id, true, None),
        Err(AccountError::NotFound)
    );
    assert_eq!(
        received.recv().unwrap(),
        format!("GET /v1/community/skins/{AI}?fields=moderation,origin&include=category&exclude=ai HTTP/1.1")
    );
    assert_eq!(
        client.community_skin(id, true, None),
        Err(AccountError::NotFound)
    );
    received.recv().unwrap();
    assert_eq!(
        client.download_community_skin(id, true, &token()),
        Err(AccountError::NotFound)
    );
    assert_eq!(
        received.recv().unwrap(),
        format!("POST /v1/community/skins/{AI}/download?exclude=ai HTTP/1.1")
    );
}

#[test]
fn an_excluding_detail_without_a_user_origin_is_not_found() {
    // 来源不明、没带 `origin` 的详情同样按不存在处理，与服务端只留 `user` 的语义一致。
    for (id, origin) in [(UNKNOWN, Some(SkinOrigin::Unknown)), (MISSING, None)] {
        let (server, _received) = serve(vec![(
            "200 OK",
            HEADER,
            serde_json::to_vec(&skin(id, origin)).unwrap(),
        )]);
        assert_eq!(
            client(&server).community_skin(Uuid::parse_str(id).unwrap(), true, None),
            Err(AccountError::NotFound)
        );
    }
}

#[test]
fn an_excluding_detail_and_download_pass_user_skins_through() {
    let id = Uuid::parse_str(USER).unwrap();
    let (origin, _received) = serve(vec![
        (
            "200 OK",
            HEADER,
            serde_json::to_vec(&skin(USER, Some(SkinOrigin::User))).unwrap(),
        ),
        (
            "200 OK",
            HEADER,
            serde_json::to_vec(
                &serde_json::json!({ "design": TouchKeyboardSkinDesign::default() }),
            )
            .unwrap(),
        ),
    ]);
    let client = client(&origin);
    assert_eq!(
        client.community_skin(id, true, None).unwrap(),
        skin(USER, Some(SkinOrigin::User))
    );
    assert_eq!(
        client.download_community_skin(id, true, &token()).unwrap(),
        TouchKeyboardSkinDesign::default()
    );
}

#[test]
fn a_list_that_does_not_exclude_is_unchanged_and_needs_no_header() {
    let (origin, received) = serve(vec![(
        "200 OK",
        "",
        page(vec![skin(USER, None), skin(AI, Some(SkinOrigin::Ai))]),
    )]);
    // 其他宿主不排除：路径与以前相同，不要求能力头，条目原样交出。
    assert_eq!(
        client(&origin)
            .community_skins(0, "", false, None, false, None)
            .unwrap()
            .skins
            .len(),
        2
    );
    assert_eq!(
        received.recv().unwrap(),
        "GET /v1/community/skins?offset=0&q=&include=category HTTP/1.1"
    );
}

#[test]
fn the_origin_field_is_optional_and_unknown_values_read_as_unknown() {
    let mut json = serde_json::to_value(skin(USER, None)).unwrap();
    assert!(!json.as_object().unwrap().contains_key("origin"));
    json["origin"] = serde_json::json!("ai");
    assert_eq!(
        serde_json::from_value::<CommunitySkin>(json.clone())
            .unwrap()
            .origin,
        Some(SkinOrigin::Ai)
    );
    json["origin"] = serde_json::json!("model-assisted");
    assert_eq!(
        serde_json::from_value::<CommunitySkin>(json)
            .unwrap()
            .origin,
        Some(SkinOrigin::Unknown)
    );
}
