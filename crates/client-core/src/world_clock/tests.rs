use super::*;

fn at(instant: &str, zone: &str) -> ClockCard {
    let timestamp = DateTime::parse_from_rfc3339(instant).unwrap().timestamp();
    snapshot(timestamp, "UTC", 0, vec![zone.into()])
        .unwrap()
        .remove(1)
}

#[test]
fn catalog_is_complete_and_valid() {
    assert!(catalog().len() > 400);
    let mut ids = HashSet::new();
    for c in catalog() {
        assert!(ids.insert(&c.id));
        assert!(timezone(&c.zone).is_ok(), "{}", c.zone);
        assert!(Coordinates::new(c.latitude, c.longitude).is_some());
        assert_eq!(c.country.len(), 2);
    }
    let tokyo = city("Asia/Tokyo").unwrap();
    assert!((tokyo.latitude - 35.6544).abs() < 0.01);
    assert!((tokyo.longitude - 139.7447).abs() < 0.01);
}

#[test]
fn dst_boundaries_and_fractional_offsets() {
    assert_eq!(
        at("2026-03-08T06:59:59Z", "America/New_York").time,
        "01:59:59"
    );
    let next = at("2026-03-08T07:00:00Z", "America/New_York");
    assert_eq!(next.time, "03:00:00");
    assert_eq!(next.offset, "UTC-04:00");
    assert_eq!(
        at("2026-11-01T05:59:59Z", "America/New_York").time,
        "01:59:59"
    );
    assert_eq!(
        at("2026-11-01T06:00:00Z", "America/New_York").time,
        "01:00:00"
    );
    assert_eq!(at("2026-01-01T00:00:00Z", "Asia/Kolkata").time, "05:30:00");
    assert_eq!(
        at("2026-01-01T00:00:00Z", "Asia/Kathmandu").time,
        "05:45:00"
    );
}

#[test]
fn local_dates_formats_and_system_offset() {
    let tokyo = at("2026-10-22T16:24:43Z", "Asia/Tokyo");
    assert_eq!(tokyo.date, "2026年10月23日 星期五");
    assert_eq!(
        tokyo
            .formats
            .iter()
            .find(|f| f.label == "ISO 8601")
            .unwrap()
            .text,
        "2026-10-23T01:24:43+09:00"
    );
    assert_eq!(
        tokyo
            .formats
            .iter()
            .find(|f| f.label == "星期")
            .unwrap()
            .text,
        "星期五"
    );
    let system = snapshot(1792686283, "Unknown/Zone", 19800, vec![])
        .unwrap()
        .remove(0);
    assert_eq!(system.offset, "UTC+05:30");
    assert!(system.solar_reference.is_none());
    assert!(system.sunrise.is_none());
    assert!(snapshot(i64::MAX, "UTC", 0, vec![]).is_err());
    assert!(snapshot(0, "UTC", 86400, vec![]).is_err());
}

#[test]
fn solar_events_are_for_city_local_date_and_handle_polar_days() {
    let tokyo = at("2026-10-22T16:24:43Z", "Asia/Tokyo");
    assert_eq!(tokyo.solar_reference.as_deref(), Some("Asia/Tokyo"));
    let rise = tokyo.sunrise.unwrap();
    let set = tokyo.sunset.unwrap();
    assert!(rise.as_str() > "05:30" && rise.as_str() < "06:30", "{rise}");
    assert!(set.as_str() > "16:30" && set.as_str() < "17:30", "{set}");
    let polar = at("2026-06-21T12:00:00Z", "Arctic/Longyearbyen");
    assert!(polar.sunrise.is_none());
    assert!(polar.sunset.is_none());
    let beijing = at("2026-10-22T16:24:43Z", "city:beijing");
    assert_ne!(
        beijing.sunrise,
        at("2026-10-22T16:24:43Z", "Asia/Shanghai").sunrise
    );
}

#[test]
fn store_is_idempotent_persistent_bounded_and_preserves_corruption() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    assert_eq!(
        stored(path, ClockAction::Load {}).unwrap(),
        ClockCities::default()
    );
    let added = stored(
        path,
        ClockAction::Add {
            id: "Asia/Tokyo".into(),
        },
    )
    .unwrap();
    assert_eq!(added.cities, ["Asia/Tokyo"]);
    assert_eq!(
        stored(
            path,
            ClockAction::Add {
                id: "Asia/Tokyo".into()
            }
        )
        .unwrap(),
        added
    );
    assert_eq!(stored(path, ClockAction::Load {}).unwrap(), added);
    assert!(stored(
        path,
        ClockAction::Add {
            id: "../invalid".into()
        }
    )
    .is_err());
    assert!(stored(
        path,
        ClockAction::Remove {
            id: "Asia/Tokyo".into()
        }
    )
    .unwrap()
    .cities
    .is_empty());
    for city in &catalog()[..MAX_CITIES] {
        stored(
            path,
            ClockAction::Add {
                id: city.id.clone(),
            },
        )
        .unwrap();
    }
    assert!(matches!(
        stored(
            path,
            ClockAction::Add {
                id: catalog()[MAX_CITIES].id.clone()
            }
        ),
        Err(ClockError::Limit)
    ));
    assert_eq!(
        stored(path, ClockAction::Load {}).unwrap().cities.len(),
        MAX_CITIES
    );
    std::fs::write(path.join(FILE_NAME), b"invalid").unwrap();
    assert!(matches!(
        stored(
            path,
            ClockAction::Add {
                id: "Asia/Tokyo".into()
            }
        ),
        Err(ClockError::Corrupt)
    ));
    assert_eq!(std::fs::read(path.join(FILE_NAME)).unwrap(), b"invalid");
}

#[cfg(unix)]
#[test]
fn store_rejects_file_symlinks_without_touching_target() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("synthetic-target");
    std::fs::write(&target, b"unchanged").unwrap();
    std::os::unix::fs::symlink(&target, directory.path().join(FILE_NAME)).unwrap();
    assert!(stored(directory.path(), ClockAction::Load {}).is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"unchanged");
}

#[test]
fn solar_uses_the_requested_date_across_the_international_date_line() {
    let c = city("Pacific/Apia").unwrap();
    let coordinates = Coordinates::new(c.latitude, c.longitude).unwrap();
    let tz = timezone(&c.zone).unwrap();
    for month in 1..=12 {
        let target = chrono::NaiveDate::from_ymd_opt(2026, month, 15).unwrap();
        let day = SolarDay::new(coordinates, target.pred_opt().unwrap());
        let expected_rise = zoned(day.event_time(SolarEvent::Sunrise).unwrap(), &tz).unwrap();
        let expected_set = zoned(day.event_time(SolarEvent::Sunset).unwrap(), &tz).unwrap();
        assert_eq!(expected_rise.date_naive(), target);
        assert_eq!(expected_set.date_naive(), target);
        let card = at(&format!("2026-{month:02}-15T00:00:00Z"), "Pacific/Apia");
        assert_eq!(
            card.sunrise,
            Some(expected_rise.format("%H:%M").to_string()),
            "{target}"
        );
        assert_eq!(
            card.sunset,
            Some(expected_set.format("%H:%M").to_string()),
            "{target}"
        );
    }
}

#[test]
fn requests_reject_unknown_fields_even_without_parameters() {
    for operation in ["catalog", "load"] {
        assert!(
            serde_json::from_value::<ClockAction>(serde_json::json!({"operation": operation}))
                .is_ok()
        );
        assert!(serde_json::from_value::<ClockAction>(
            serde_json::json!({"operation": operation, "unexpected": true})
        )
        .is_err());
    }
}
