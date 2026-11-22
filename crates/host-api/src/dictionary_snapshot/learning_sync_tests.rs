use super::{download_into, downloading_device};

#[test]
fn cloud_activation_keeps_original_learning_times_and_all_fixed_candidates() {
    let device = downloading_device();
    device.sql(
        "UPDATE user_dictionary_operations SET updated_at=123 WHERE user_inserted=0;
         INSERT INTO fixed_candidate_position_state VALUES
         ('ni''hao','ni''hao','拟好',2,127),('ni''hao','ni''hao','妮好',2,126);
         INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at)
         VALUES('english','synthetic','Synthetic','upsert',23000000000,'Synthetic',0,125);",
    );
    download_into(&device, true);
    for query in [
        "SELECT count(*)=2 FROM user_dictionary_operations WHERE user_inserted=0 AND updated_at=123 AND value IN ('拟好','妮好')",
        "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='english' AND weight=23000000000 AND updated_at=125",
        "SELECT count(*)=2 FROM fixed_candidate_position_state WHERE context_key='ni''hao' AND updated_at IN (126,127)",
        "SELECT count(*) FROM fixed_candidate_positions WHERE context_key='ni''hao' AND value='拟好' AND position=2",
        "SELECT count(*) FROM pinned_candidates WHERE value='拟好' AND updated_at=100",
    ] {
        assert_eq!(device.number(query), Some(1), "{query}");
    }
    assert!(super::lost_learning(&device).is_empty());
    assert_eq!(device.number("SELECT count(*) FROM user_dictionary_operations WHERE value='泥好' AND user_inserted=1 AND operation='upsert'"), Some(1));
}

#[test]
fn activation_without_keep_local_learning_does_not_carry_hidden_candidates() {
    let device = downloading_device();
    device.sql(
        "INSERT INTO fixed_candidate_position_state VALUES
         ('ni''hao','ni''hao','拟好',2,127),('ni''hao','ni''hao','妮好',2,126);",
    );
    download_into(&device, false);
    assert_eq!(
        device.number("SELECT count(*) FROM fixed_candidate_position_state"),
        Some(0)
    );
}
