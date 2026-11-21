use super::group::Group;
use kokoda_transport::tls;
use std::{collections::BTreeMap, net::TcpListener, path::Path, sync::Arc};
fn clipboard_fields(
    content: Option<super::clipboard::Content>,
) -> (Option<String>, Option<String>) {
    use base64::{engine::general_purpose::STANDARD, Engine};
    match content {
        Some(super::clipboard::Content::Text(s)) => (Some(s), None),
        Some(super::clipboard::Content::Png(png)) => (None, Some(STANDARD.encode(png))),
        None => (None, None),
    }
}
type Reply = (serde_json::Value, Option<Arc<Vec<u8>>>);

fn frame_header(
    id: u64,
    token: u64,
    caret: Option<(f64, f64)>,
    frame: &super::live_capture::EncodedFrame,
    sid: u64,
    event: bool,
) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD, Engine};
    serde_json::json!({"type":"frame","event":event,"session_id":sid,"id":id,"token":token,
        "sequence":frame.sequence,"stream_epoch":frame.epoch,"width":frame.width,"height":frame.height,"format":frame.format,
        "caret":caret,"encoded_width":frame.encoded_width,"encoded_height":frame.encoded_height,
        "encoder":frame.encoder,"encode_ms":frame.encode_ms,
        "alpha_png":frame.alpha_png.as_ref().map(|png| STANDARD.encode(png.as_slice())),
        "color_space":if frame.format == "h264" {"srgb-bt601-limited"} else {"image"}})
}

pub fn serve(
    address: &str,
    identity: &Path,
    sharing: Arc<super::sharing::Sharing>,
) -> tls::Result<()> {
    let config = tls::server_config(identity)?;
    let token = tls::pairing_token(identity)?;
    serve_authenticated(
        address,
        sharing,
        move |socket| tls::accept(socket, config.clone(), &token),
        false,
    )
}

pub fn serve_password(
    address: &str,
    record: &Path,
    sharing: Arc<super::sharing::Sharing>,
) -> tls::Result<()> {
    let bytes = kokoda_platform_windows::credentials::load(record)?;
    let credentials = kokoda_transport::password::Credentials::decode(&bytes)?;
    serve_authenticated(
        address,
        sharing,
        move |socket| kokoda_transport::password::accept(socket, &credentials),
        true,
    )
}
fn serve_authenticated(
    address: &str,
    sharing: Arc<super::sharing::Sharing>,
    accept: impl Fn(std::net::TcpStream) -> tls::Result<tls::Server>,
    password_mode: bool,
) -> tls::Result<()> {
    super::windows::require_interactive_session()?;
    let mut limiter = kokoda_transport::password::AttemptLimiter::default();
    let listener = TcpListener::bind(address)?;
    let clipboard = super::clipboard::Clipboard::new()?;
    println!("Kokoda listening on {}", listener.local_addr()?);
    // One authenticated controller at a time. Resume state survives transport loss,
    // never a service restart, and never includes queued input or clipboard data.
    let mut resumes: BTreeMap<
        u64,
        (
            String,
            super::input::Target,
            serde_json::Value,
            std::time::Instant,
        ),
    > = BTreeMap::new();
    let instance = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let mut resume_serial = 0u64;
    let mut admission = crate::Admission::default();
    for socket in listener.incoming() {
        let mut admitted = None;
        let result = (|| -> tls::Result<()> {
            let socket = socket?;
            if password_mode && !limiter.allow(std::time::Instant::now(), socket.peer_addr()?.ip())
            {
                return Err("Authentication attempt limit reached".into());
            }
            let mut connection = accept(socket)?;
            admitted = Some(admission.admit().map_err(|_| "Host busy")?);
            connection.stream_mut().sock.set_nonblocking(true)?;
            let mut groups: BTreeMap<u64, Group> = BTreeMap::new();
            let mut clipboard_sequences: BTreeMap<u64, u32> = BTreeMap::new();
            let mut input_session: Option<u64> = None;
            let mut push_video = false;
            let mut last_request = std::time::Instant::now();
            let mut push_cursor = 0usize;
            let mut pending_frame: Option<(u64, u64, u64)> = None;
            loop {
                if sharing.restricted()
                    && pending_frame.is_some_and(|(sid, id, token)| {
                        !groups
                            .get(&sid)
                            .is_some_and(|g| g.contains_member(id, Some(token)))
                    })
                {
                    // A partially written framed record cannot be safely removed.
                    // End the connection rather than finish a revoked payload.
                    return Err("Queued window sharing permission revoked".into());
                }
                connection.flush_pending()?;
                if !connection.has_pending_write() {
                    pending_frame = None;
                }
                if push_video && !connection.has_pending_write() {
                    let ids: Vec<_> = groups.keys().copied().collect();
                    for offset in 0..ids.len() {
                        let index = (push_cursor + offset) % ids.len();
                        let sid = ids[index];
                        let result = groups.get_mut(&sid).unwrap().next_frame();
                        match result {
                            Ok(Some((id, token, caret, frame))) => {
                                let header = frame_header(id, token, caret, &frame, sid, true);
                                connection.queue_send(&serde_json::to_vec(&header)?)?;
                                connection.queue_send(&frame.data)?;
                                pending_frame = Some((sid, id, token));
                                push_cursor = (index + 1) % ids.len();
                                break;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                groups.remove(&sid);
                                resumes.remove(&sid);
                                let response = serde_json::json!({"type":"error","event":true,"session_id":sid,"selection_lost":true,"message":error.to_string()});
                                connection.queue_send(&serde_json::to_vec(&response)?)?;
                            }
                        }
                    }
                }
                let packet = match connection.receive() {
                    Ok(packet) => {
                        last_request = std::time::Instant::now();
                        packet
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                    {
                        if last_request.elapsed() > std::time::Duration::from_secs(30) {
                            return Err("Controller heartbeat expired".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                };
                let mut request: serde_json::Value = serde_json::from_slice(&packet)?;
                // Restricted viewing never touches global input or clipboard state.
                if sharing.restricted()
                    && matches!(
                        request["command"].as_str(),
                        Some("input" | "clipboard" | "clipboard_poll" | "window_action")
                    )
                {
                    connection.queue_send(&serde_json::to_vec(&serde_json::json!({"type":"error","code":"sharing_read_only","message":"Restricted sharing is read-only","session_id":request.get("session_id"),"request_id":request.get("request_id")}))?)?;
                    continue;
                }
                if let Err(code) = kokoda_protocol::validate(&request) {
                    connection.queue_send(&serde_json::to_vec(&serde_json::json!({"type":"error", "code":code, "message":code, "session_id":request.get("session_id").and_then(|v|v.as_u64()).unwrap_or(0)}))?)?;
                    continue;
                }
                let sid = request
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if request
                    .get("session_id")
                    .is_some_and(|v| v.as_u64().is_none())
                {
                    connection.queue_send(&serde_json::to_vec(
                        &serde_json::json!({"type":"error","message":"Invalid session ID"}),
                    )?)?;
                    continue;
                }
                let is_resume =
                    request.get("command").and_then(|v| v.as_str()) == Some("resume_window");
                if is_resume {
                    let ticket = request.get("resume_token").and_then(|v| v.as_str());
                    let valid = resumes.get(&sid).is_some_and(|(saved, target, _, time)| {
                        Some(saved.as_str()) == ticket
                            && time.elapsed() < std::time::Duration::from_secs(300)
                            && target.validate_identity().is_ok()
                    });
                    if !valid {
                        connection.queue_send(&serde_json::to_vec(&serde_json::json!({"type":"error","session_id":sid,"command":"resume_window","selection_lost":true,
                            "message":"原窗口无法恢复或主机服务已重启，请重新选择窗口。"}))?)?;
                        continue;
                    }
                    request = resumes.get(&sid).unwrap().2.clone();
                }
                let command = request
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let window_id = request.get("id").and_then(|v| v.as_u64());
                let generation = request.get("token").and_then(|v| v.as_u64());
                let only_release = command == "input"
                    && request
                        .get("events")
                        .and_then(|v| v.as_array())
                        .is_some_and(|events| {
                            events
                                .iter()
                                .all(|e| e.get("kind").and_then(|v| v.as_str()) == Some("release"))
                        });
                if matches!(command, "input" | "clipboard")
                    && !only_release
                    && input_session != Some(sid)
                {
                    for group in groups.values_mut() {
                        group.release_inputs();
                    }
                    input_session = Some(sid);
                }
                let mut group = groups.remove(&sid);
                let mut clipboard_sequence = *clipboard_sequences
                    .get(&sid)
                    .unwrap_or(&clipboard.sequence());
                let response = (|| -> tls::Result<Reply> {
                    if matches!(
                        command,
                        "input"
                            | "frame"
                            | "video_settings"
                            | "video_viewport"
                            | "video_feedback"
                            | "window_action"
                            | "clipboard"
                            | "clipboard_poll"
                    ) && window_id.is_some()
                        && generation.is_none()
                    {
                        return Err("Expected window generation token".into());
                    }
                    let response = match command {
                        "start_stream" => {
                            push_video = true;
                            serde_json::json!({"type":"stream_started"})
                        }
                        "ping" => serde_json::json!({"type":"pong"}),
                        "close_window" => {
                            drop(group.take());
                            resumes.remove(&sid);
                            serde_json::json!({"type":"closed"})
                        }
                        "list_windows" => {
                            serde_json::json!({"type":"windows", "multi_session":true,"window_video_alpha":true,"anchored_pointer":true,"window_management":true,"resolution_bitrate":true,"playback_feedback":true,"push_video":true,"live_video_settings":true,"max_sessions":8,"read_only":sharing.restricted(),"windows":kokoda_platform_windows::catalog::describe(sharing.catalog()?)})
                        }
                        "select_window" => {
                            if groups.len() >= kokoda_protocol::MAX_SESSIONS {
                                return Err("At most 8 application sessions are supported".into());
                            }
                            drop(group.take());
                            if !is_resume {
                                resumes.remove(&sid);
                            }
                            let id = window_id.ok_or("Expected window ID")?;
                            let application = request.get("scope").and_then(|v| v.as_str())
                                == Some("application");
                            if request["playback_feedback"] == true && !push_video {
                                return Err("Playback feedback requires push streaming".into());
                            }
                            let selected = Group::new(
                                id,
                                sharing.clone(),
                                application,
                                super::video::VideoOptions::parse(&request)?,
                                request
                                    .get("alpha_video")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                request
                                    .get("playback_feedback")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                request["window_management"] == true,
                            )?;
                            for other in groups.values() {
                                if other.contains_visible(id, None)
                                    || selected.contains_visible(other.root(), None)
                                {
                                    return Err("This window already belongs to another application session".into());
                                }
                            }
                            let target = super::input::Target::new(id)?;
                            resume_serial += 1;
                            let ticket = format!("{instance}-{resume_serial}");
                            resumes.insert(
                                sid,
                                (
                                    ticket.clone(),
                                    target,
                                    request.clone(),
                                    std::time::Instant::now(),
                                ),
                            );
                            if resumes.len() > 32 {
                                if let Some(old) = resumes
                                    .iter()
                                    .filter(|(key, _)| **key != sid && !groups.contains_key(key))
                                    .min_by_key(|(_, v)| v.3)
                                    .map(|(key, _)| *key)
                                {
                                    resumes.remove(&old);
                                }
                            }
                            let response = serde_json::json!({"type":"selected","read_only":sharing.restricted(),"id":id,"title":selected.title(),
                                "fps":request.get("fps"),"bitrate":request.get("bitrate"),"resume_token":ticket,"scope":if application {"application"} else {"window"}, "window_groups":true,"codec":request.get("codec").and_then(|v|v.as_str()).unwrap_or("images")});
                            clipboard_sequence = clipboard.sequence();
                            group = Some(selected);
                            response
                        }
                        "window_action" => {
                            let id = window_id.ok_or("Expected window ID")?;
                            let token = generation.ok_or("Expected window generation token")?;
                            group
                                .as_mut()
                                .ok_or("Select a window first")?
                                .window_action(id, token, &request)?
                        }
                        "window_group" => {
                            group.as_mut().ok_or("Select a window first")?.snapshot()?
                        }
                        "video_feedback" => {
                            let selected = group.as_mut().ok_or("Select a window first")?;
                            let number = |key| {
                                request
                                    .get(key)
                                    .and_then(|v| v.as_u64())
                                    .ok_or("Missing feedback identity")
                            };
                            selected.feedback(
                                window_id.ok_or("Expected window ID")?,
                                generation.ok_or("Expected window generation")?,
                                number("stream_epoch")?,
                                number("sequence")?,
                                request["decode_ms"].as_f64().ok_or("Missing decode time")?,
                                request["queue_ms"].as_f64().ok_or("Missing queue time")?,
                            )?
                        }
                        "video_settings" => {
                            let selected = group.as_mut().ok_or("Select a window first")?;
                            let epoch = selected.set_video(
                                window_id.ok_or("Expected window ID")?,
                                generation.ok_or("Expected generation token")?,
                                super::video::VideoOptions::parse(&request)?,
                            )?;
                            // Resume must restore the most recently applied settings.
                            if let Some((_, _, saved, _)) = resumes.get_mut(&sid) {
                                for key in ["codec", "fps", "bitrate", "max_width", "encoder"] {
                                    saved.as_object_mut().unwrap().remove(key);
                                    if let Some(value) = request.get(key) {
                                        saved[key] = value.clone();
                                    }
                                }
                            }
                            serde_json::json!({"type":"video_settings_ok","codec":request.get("codec").and_then(|v|v.as_str()).unwrap_or("images"),"stream_epoch":epoch})
                        }
                        "video_viewport" => {
                            let selected = group.as_mut().ok_or("Select a window first")?;
                            let id = window_id.ok_or("Expected window ID")?;
                            let token = generation.ok_or("Expected window generation token")?;
                            let dimension = |key| -> tls::Result<u32> {
                                let n = request
                                    .get(key)
                                    .and_then(|v| v.as_u64())
                                    .ok_or("Expected viewport dimension")?;
                                Ok(u32::try_from(n)?)
                            };
                            selected.set_viewport(
                                id,
                                token,
                                dimension("width")?,
                                dimension("height")?,
                            )?;
                            serde_json::json!({"type":"viewport_ok"})
                        }
                        "clipboard_poll" => {
                            let id = window_id.ok_or("Expected window ID")?;
                            let token = generation.ok_or("Expected window generation token")?;
                            if request.get("discard").and_then(|v| v.as_bool()) == Some(true) {
                                let selected = group.as_ref().ok_or("Select a window first")?;
                                if !selected.contains_visible(id, Some(token)) {
                                    return Err("Window unavailable".into());
                                }
                                clipboard_sequence = clipboard.sequence();
                            }
                            let (sequence, text) =
                                group
                                    .as_mut()
                                    .ok_or("Select a window first")?
                                    .poll_clipboard(id, token, clipboard_sequence, &clipboard)?;
                            clipboard_sequence = sequence;
                            let (text, image_png) = clipboard_fields(text?);
                            serde_json::json!({"type":"clipboard_result","window_id":id,"token":token,
                                "request_id":request.get("request_id"),"action":"poll","text":text,"image_png":image_png})
                        }
                        "clipboard" => {
                            let id = window_id.ok_or("Expected window ID")?;
                            let token = generation.ok_or("Expected window generation token")?;
                            let action = request
                                .get("action")
                                .and_then(|v| v.as_str())
                                .ok_or("Expected clipboard action")?;
                            let text = group.as_mut().ok_or("Select a window first")?.clipboard(
                                id,
                                token,
                                action,
                                request.get("text").and_then(|v| v.as_str()),
                                request.get("image_png").and_then(|v| v.as_str()),
                                &clipboard,
                            )?;
                            clipboard_sequence = clipboard.sequence();
                            let (text, image_png) = clipboard_fields(text);
                            serde_json::json!({"type":"clipboard_result","window_id":id,"token":token,"request_id":request.get("request_id"),"action":action,"text":text,"image_png":image_png})
                        }
                        "input" => {
                            let events: Vec<kokoda_app_model::input::InputEvent> =
                                serde_json::from_value(
                                    request
                                        .get("events")
                                        .cloned()
                                        .ok_or("Missing input events")?,
                                )?;
                            if events.len() > 256 {
                                return Err("Too many input events".into());
                            }
                            let selected = group.as_mut().ok_or("Select a window first")?;
                            selected.input(
                                window_id.unwrap_or(selected.root()),
                                generation,
                                events,
                            )?;
                            serde_json::json!({"type":"input_ok"})
                        }
                        "frame" => {
                            let selected = group.as_mut().ok_or("Select a window first")?;
                            if sharing.restricted()
                                && !selected.contains_visible(
                                    window_id.unwrap_or(selected.root()),
                                    generation,
                                )
                            {
                                return Err("Window unavailable".into());
                            }
                            if push_video {
                                return Ok((
                                    serde_json::json!({"type":"stream_started","session_id":0}),
                                    None,
                                ));
                            }
                            let id = window_id.unwrap_or(selected.root());
                            let after = request.get("after").and_then(|v| v.as_u64()).unwrap_or(0);
                            let (token, caret, frame) = selected.frame(id, generation, after)?;
                            match frame {
                                Some(frame) => {
                                    return Ok((
                                        frame_header(id, token, caret, &frame, sid, false),
                                        Some(frame.data),
                                    ))
                                }
                                None => {
                                    serde_json::json!({"type":"unchanged","id":id,"token":token,"caret":caret})
                                }
                            }
                        }
                        _ => serde_json::json!({"type":"error","message":"Unknown command"}),
                    };
                    Ok((response, None))
                })();
                match response {
                    Ok((mut response, frame)) => {
                        response["session_id"] = sid.into();
                        connection.queue_send(&serde_json::to_vec(&response)?)?;
                        if let Some(frame) = frame {
                            pending_frame = Some((
                                sid,
                                response["id"].as_u64().ok_or("Missing frame window")?,
                                response["token"]
                                    .as_u64()
                                    .ok_or("Missing frame generation")?,
                            ));
                            connection.queue_send(&frame)?;
                        }
                    }
                    Err(error) => {
                        if sharing.restricted() {
                            resumes.remove(&sid);
                        }
                        if command == "input" {
                            if let Some(group) = group.as_mut() {
                                group.release_inputs();
                            }
                        }
                        // Window-scoped errors never destroy unrelated popup streams.
                        let lost =
                            matches!(command, "select_window" | "window_group") || group.is_none();
                        if lost {
                            drop(group.take());
                        }
                        let unavailable = window_id.is_some_and(|id| {
                            group
                                .as_ref()
                                .is_some_and(|g| !g.contains_member(id, generation))
                        });
                        let response = serde_json::json!({"type":"error","message":error.to_string(),
                            "session_id":sid,"command":command,"request_id":request.get("request_id"),"selection_lost":lost,"window_lost":unavailable,"window_id":window_id,"token":generation});
                        connection.queue_send(&serde_json::to_vec(&response)?)?;
                    }
                }
                if let Some(group) = group {
                    if let Some((_, target, saved, _)) = resumes.get_mut(&sid) {
                        if saved["id"].as_u64() != Some(group.root()) {
                            *target = super::input::Target::new(group.root())?;
                            saved["id"] = group.root().into();
                        }
                    }
                    groups.insert(sid, group);
                }
                if groups.contains_key(&sid) {
                    clipboard_sequences.insert(sid, clipboard_sequence);
                } else {
                    clipboard_sequences.remove(&sid);
                }
            }
        })();
        // Groups and their input/capture resources have dropped before releasing admission.
        if let Some(connection) = admitted {
            admission.release(connection);
        }
        if let Err(error) = result {
            for (_, _, _, time) in resumes.values_mut() {
                *time = std::time::Instant::now();
            }
            eprintln!("Session ended: {error}");
        }
    }
    Ok(())
}
