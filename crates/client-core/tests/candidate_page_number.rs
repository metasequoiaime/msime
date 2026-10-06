use msime_client_core::host_surface::{HostCapabilities, HostPlatform};
use msime_client_core::preferences::{Preferences, PreferencesStore};

#[test]
fn legacy_preferences_show_page_numbers_and_the_toggle_roundtrips() {
    let mut legacy = serde_json::to_value(Preferences::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("show_candidate_page_number");
    let mut preferences: Preferences = serde_json::from_value(legacy).unwrap();
    assert!(preferences.show_candidate_page_number);
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    preferences.show_candidate_page_number = false;
    store.save(0, preferences).unwrap();
    let snapshot = store.load().unwrap();
    assert!(!snapshot.preferences.show_candidate_page_number);
    let mut restored = snapshot.preferences;
    restored.show_candidate_page_number = true;
    store.save(snapshot.revision, restored).unwrap();
    assert!(store.load().unwrap().preferences.show_candidate_page_number);
}

#[test]
fn only_linux_exposes_the_page_number_control() {
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).candidate_page_number,
            platform == HostPlatform::Linux
        );
    }
}
