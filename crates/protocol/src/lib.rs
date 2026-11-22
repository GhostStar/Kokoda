//! Kokoda development wire contract. This is not a RustDesk protocol endpoint.
use kokoda_app_model::Quality;
use serde::Deserialize;

pub const VERSION: &str = "kokoda/dev-1";
pub const AUTHENTICATION_FAILED: &[u8] = b"kokoda/dev-1/authentication-failed";
pub const MAX_INPUT_EVENTS: usize = 256;
pub const MAX_SESSIONS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    ListWindows,
    SelectWindow,
    ResumeWindow,
    CloseWindow,
    WindowGroup,
    WindowAction,
    StartStream,
    Ping,
    VideoSettings,
    VideoViewport,
    VideoFeedback,
    Clipboard,
    ClipboardPoll,
    Input,
    Frame,
}

/// Validate routing fields before platform dispatch. Native adapters validate payload values.
#[derive(Debug, Deserialize)]
pub struct Request {
    pub command: Command,
    #[serde(default)]
    pub session_id: u64,
    pub id: Option<u64>,
    pub token: Option<u64>,
}

pub fn validate(value: &serde_json::Value) -> Result<Request, String> {
    let request: Request =
        serde_json::from_value(value.clone()).map_err(|_| "invalid_request".to_string())?;
    if matches!(
        request.command,
        Command::Input
            | Command::WindowAction
            | Command::Frame
            | Command::VideoSettings
            | Command::VideoViewport
            | Command::VideoFeedback
            | Command::Clipboard
            | Command::ClipboardPoll
    ) && request.id.is_some()
        && request.token.is_none()
    {
        return Err("missing_window_generation".into());
    }
    if request.command == Command::WindowAction && (request.id.is_none() || request.token.is_none())
    {
        return Err("missing_window_generation".into());
    }
    if request.command == Command::Input {
        let events = value
            .get("events")
            .and_then(|v| v.as_array())
            .ok_or("missing_input_events")?;
        if events.len() > MAX_INPUT_EVENTS {
            return Err("too_many_input_events".into());
        }
        serde_json::from_value::<Vec<kokoda_app_model::input::InputEvent>>(value["events"].clone())
            .map_err(|_| "invalid_input_events")?;
    }
    Ok(request)
}

/// One definition for all frontend presets. Native renderers do not choose policy.
pub fn quality_profile(quality: Quality) -> serde_json::Value {
    use serde_json::json;
    match quality {
        Quality::Original => json!({}),
        Quality::Smooth => json!({"codec":"h264","fps":30,"bitrate":2_000_000,"max_width":1280}),
        Quality::Clear => json!({"codec":"h264","fps":30,"bitrate":6_000_000,"max_width":1920}),
        Quality::HighFrameRate => {
            json!({"codec":"h264","fps":60,"bitrate":8_000_000,"max_width":1920})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_bad_routing_and_unbounded_input_before_dispatch() {
        for v in [
            json!({"command":"input","session_id":-1}),
            json!({"command":"unknown"}),
            json!({"command":"window_action","id":1,"action":"close"}),
            json!({"command":"window_action","token":1,"action":"resize"}),
            json!({"command":"frame","id":1}),
            json!({"command":"clipboard","token":"1"}),
            json!({"command":"input","events":vec![json!({"kind":"release"});257]}),
        ] {
            assert!(validate(&v).is_err());
        }
        assert!(validate(
            &json!({"command":"input","id":1,"token":2,"events":[{"kind":"text","text":"中文🙂"}]})
        )
        .is_ok());
    }
    #[test]
    fn positioned_wheel_requires_a_complete_point() {
        assert!(validate(&json!({"command":"input","events":[{"kind":"wheel_at","x":0.3,"y":0.4,"width":800,"height":600,"delta":120}]})).is_ok());
        assert!(
            validate(&json!({"command":"input","events":[{"kind":"wheel_at","delta":120}]}))
                .is_err()
        );
        assert!(
            validate(&json!({"command":"input","events":[{"kind":"wheel","delta":120}]})).is_ok()
        );
    }
    #[test]
    fn profiles_match_cross_platform_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/contracts/video-profiles.json"))
                .unwrap();
        assert_eq!(fixture["defaultIndex"], 2);
        for (index, quality) in [
            Quality::Original,
            Quality::Smooth,
            Quality::Clear,
            Quality::HighFrameRate,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(quality_profile(quality), fixture["profiles"][index]);
        }
    }
}
