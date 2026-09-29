use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

const MAX_CAPTURE_DEVICES: usize = 256;

#[derive(Serialize)]
pub struct CaptureDevice {
    backend: &'static str,
    id: String,
    label: String,
}

fn add(devices: &mut Vec<CaptureDevice>, backend: &'static str, id: &str, label: &str) {
    if devices.len() >= MAX_CAPTURE_DEVICES
        || id.is_empty()
        || id.len() > 512
        || id.chars().count() > 128
        || msime_client_core::has_disallowed_control_with_options(id, false)
        || devices
            .iter()
            .any(|device| device.backend == backend && device.id == id)
    {
        return;
    }
    let label: String = label
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect();
    devices.push(CaptureDevice {
        backend,
        id: id.to_owned(),
        label: if label.is_empty() {
            id.to_owned()
        } else {
            label
        },
    });
}

fn sort_devices(devices: &mut [CaptureDevice]) {
    // Keep the picker stable even when pactl/pw-dump/arecord enumerate in a
    // different order. Backend order follows the explicit Linux capture
    // choices in the settings UI; IDs are stable endpoint identities rather
    // than display labels, so duplicate labels remain selectable.
    fn backend_rank(backend: &str) -> u8 {
        match backend {
            "pulse" => 0,
            "pipewire" => 1,
            "alsa" => 2,
            _ => 3,
        }
    }
    devices.sort_by(|left, right| {
        backend_rank(left.backend)
            .cmp(&backend_rank(right.backend))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.label.cmp(&right.label))
    });
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    super::linux_process::read_text(program, args, 1024 * 1024, Duration::from_secs(2))
}

fn pipewire_value_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|id| id.to_string()))
}

fn pipewire_devices(document: &Value) -> Vec<CaptureDevice> {
    let Some(nodes) = document.as_array() else {
        return Vec::new();
    };
    let mut devices = Vec::with_capacity(nodes.len().min(MAX_CAPTURE_DEVICES));
    for node in nodes {
        if node.get("type").and_then(Value::as_str) != Some("PipeWire:Interface:Node") {
            continue;
        }
        let props = &node["info"]["props"];
        if !matches!(
            props["media.class"].as_str(),
            Some("Audio/Source" | "Audio/Source/Virtual")
        ) {
            continue;
        }
        // pw-cat accepts either a node name or object.serial. Some PipeWire
        // sources expose only the latter, so do not hide an otherwise usable
        // source from the settings picker.
        let id = props["node.name"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| pipewire_value_id(&props["object.serial"]));
        let Some(id) = id else {
            continue;
        };
        let label = props["node.description"]
            .as_str()
            .or_else(|| props["node.nick"].as_str())
            .unwrap_or(&id);
        add(&mut devices, "pipewire", &id, label);
    }
    devices
}

pub fn list() -> Vec<CaptureDevice> {
    let mut devices = Vec::new();
    if let Some(Value::Array(sources)) = output("pactl", &["--format=json", "list", "sources"])
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        devices.reserve(sources.len().min(MAX_CAPTURE_DEVICES));
        for source in sources {
            let Some(id) = source.get("name").and_then(Value::as_str) else {
                continue;
            };
            // Monitor sources record playback, rather than microphone input.
            if id.ends_with(".monitor") {
                continue;
            }
            add(
                &mut devices,
                "pulse",
                id,
                source
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(id),
            );
        }
    }
    if let Some(document) = output("pw-dump", &[]).and_then(|text| serde_json::from_str(&text).ok())
    {
        devices.extend(pipewire_devices(&document));
    }
    if let Some(text) = output("arecord", &["-L"]) {
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if line.is_empty() || line.starts_with(char::is_whitespace) || line == "null" {
                continue;
            }
            let label = lines
                .peek()
                .filter(|next| next.starts_with(char::is_whitespace))
                .map(|next| next.trim())
                .unwrap_or(line);
            add(&mut devices, "alsa", line, label);
        }
    }
    sort_devices(&mut devices);
    devices
}

#[cfg(test)]
mod tests {
    use super::{pipewire_devices, sort_devices, CaptureDevice};
    use serde_json::json;

    #[test]
    fn pipewire_sources_prefer_name_and_fall_back_to_serial() {
        let devices = pipewire_devices(&json!([
            {
                "id": 11,
                "type": "PipeWire:Interface:Node",
                "info": { "props": {
                    "media.class": "Audio/Source",
                    "node.name": "alsa_input.synthetic",
                    "object.serial": "101",
                    "node.description": "Synthetic microphone"
                }}
            },
            {
                "id": 12,
                "type": "PipeWire:Interface:Node",
                "info": { "props": {
                    "media.class": "Audio/Source/Virtual",
                    "object.serial": 202,
                    "node.nick": "Virtual source"
                }}
            },
            {
                "id": 13,
                "type": "PipeWire:Interface:Node",
                "info": { "props": { "media.class": "Audio/Source" }}
            },
            {
                "id": 14,
                "type": "PipeWire:Interface:Node",
                "info": { "props": { "media.class": "Audio/Sink" }}
            }
        ]));

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "alsa_input.synthetic");
        assert_eq!(devices[1].id, "202");
        assert_eq!(devices[1].label, "Virtual source");
    }

    #[test]
    fn pipewire_devices_reject_non_arrays() {
        assert!(pipewire_devices(&json!({})).is_empty());
    }

    #[test]
    fn device_order_is_deterministic_and_backend_grouped() {
        let mut devices = vec![
            CaptureDevice {
                backend: "alsa",
                id: "hw:0".into(),
                label: "ALSA".into(),
            },
            CaptureDevice {
                backend: "pulse",
                id: "z-source".into(),
                label: "Z".into(),
            },
            CaptureDevice {
                backend: "pulse",
                id: "a-source".into(),
                label: "A".into(),
            },
            CaptureDevice {
                backend: "pipewire",
                id: "node-2".into(),
                label: "Node 2".into(),
            },
        ];
        sort_devices(&mut devices);
        let keys: Vec<_> = devices
            .iter()
            .map(|device| (device.backend, device.id.as_str()))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("pulse", "a-source"),
                ("pulse", "z-source"),
                ("pipewire", "node-2"),
                ("alsa", "hw:0"),
            ]
        );
    }
}
