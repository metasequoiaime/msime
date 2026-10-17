//! `preferences_put`（#8201 的 P4）：整份替换云端设置文档，按修订号比较并交换，可以绑定账号。

use super::*;

fn put(revision: i64, user_id: Option<&str>) -> Value {
    let mut operation = json!({
        "operation": "preferences_put",
        "document": { "input.schema": "wubi", "input.learning": true, "input.page_size": 7 },
        "revision": revision,
    });
    if let Some(user_id) = user_id {
        operation["user_id"] = json!(user_id);
    }
    operation
}

#[test]
fn the_document_replaces_the_cloud_one_only_at_the_revision_it_was_read_at() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture.run(put(0, None)).unwrap(),
        json!({
            "revision": 1,
            "settings": { "input.learning": true, "input.page_size": 7, "input.schema": "wubi" },
        })
    );
    // 云端已经是修订号 1，再拿 0 去写是冲突，不重试。
    assert_eq!(fixture.code(put(0, None)), "account_conflict");
    assert_eq!(fixture.run(put(1, None)).unwrap()["revision"], 2);
    assert_eq!(
        fixture.api.calls(),
        [
            "put_preferences a",
            "put_preferences a",
            "put_preferences a"
        ]
    );
}

#[test]
fn a_bound_upload_goes_only_to_that_account() {
    let fixture = Fixture::signed_in();
    assert_eq!(
        fixture.code(put(0, Some("someone-else"))),
        "account_cancelled"
    );
    assert!(fixture.api.calls().is_empty());
    assert_eq!(
        fixture.run(put(0, Some(ACCOUNT_USER))).unwrap()["revision"],
        1
    );
    assert_eq!(fixture.api.calls(), ["put_preferences a"]);
}

#[test]
fn the_answer_is_dropped_when_the_account_signs_out_during_the_request() {
    let fixture = Fixture::signed_in();
    let serving = Arc::downgrade(&fixture.account);
    *fixture.api.during_request.lock().unwrap() = Some(Box::new(move || {
        serving.upgrade().unwrap().forget().unwrap()
    }));
    assert_eq!(
        fixture.code(put(0, Some(ACCOUNT_USER))),
        "account_cancelled"
    );
    assert_eq!(fixture.api.calls(), ["put_preferences a"]);
}

#[test]
fn the_upload_does_not_fall_back_to_the_anonymous_account() {
    let fixture = Fixture::new(None, Some(saved('1', '2', ANONYMOUS_USER)));
    assert_eq!(fixture.code(put(0, None)), "account_unauthorized");
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn a_rejected_token_is_refreshed_once_and_the_upload_sent_again() {
    let fixture = Fixture::signed_in();
    fixture.api.stale.lock().unwrap().push(token('a'));
    assert_eq!(
        fixture.run(put(0, Some(ACCOUNT_USER))).unwrap()["revision"],
        1
    );
    assert_eq!(
        fixture.api.calls(),
        ["put_preferences a", "refresh b", "put_preferences c"]
    );
}

#[test]
fn bad_documents_are_refused_before_any_backend_call() {
    let fixture = Fixture::signed_in();
    let mut negative = put(0, None);
    negative["revision"] = json!(-1);
    let mut bad_key = put(0, None);
    bad_key["document"] = json!({ "input schema": "wubi" });
    let mut empty_user = put(0, None);
    empty_user["user_id"] = json!("");
    for operation in [negative, bad_key, empty_user] {
        assert_eq!(
            fixture.code(operation.clone()),
            "account_invalid",
            "{operation}"
        );
    }
    // 服务端整个请求体最多 1 MiB：单个值没超过本机的上限，整份文档超过了。
    let mut large = put(0, None);
    large["document"] = json!({ "input.custom_prompt": "a".repeat(1_048_570) });
    assert_eq!(fixture.code(large), "account_too_large");
    assert!(fixture.api.calls().is_empty());
}
