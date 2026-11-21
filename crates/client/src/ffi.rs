//! Small C ABI for native runners. No platform objects cross this boundary.
use crate::{Action, Connection, Controller, Quality};
use kokoda_app_model::Window;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_void, CStr, CString},
};

#[derive(Default)]
struct Bridge {
    controller: Controller,
    connection: Option<Connection>,
    sessions: BTreeMap<u64, Window>,
    preset: u64,
    discovery: Option<kokoda_discovery::Browser>,
}
impl Bridge {
    fn new() -> Self {
        Self {
            preset: 2,
            ..Default::default()
        }
    }
    fn dispatch(&mut self, event: Value) -> Result<Value, &'static str> {
        let mut actions = Vec::new();
        match event["type"].as_str().ok_or("missing_action")? {
            "discovery_start" => {
                if self.discovery.is_none() {
                    match kokoda_discovery::Browser::new() {
                        Ok(browser) => self.discovery = Some(browser),
                        Err(_) => return Ok(json!({"hosts":[],"unavailable":true})),
                    }
                }
                return Ok(json!({"hosts":self.discovery.as_mut().unwrap().snapshot()}));
            }
            "discovery_poll" => {
                return Ok(
                    json!({"hosts":self.discovery.as_mut().map(|b|b.snapshot()).unwrap_or_default()}),
                )
            }
            "discovery_stop" => {
                self.discovery = None;
                return Ok(json!({"hosts":[]}));
            }
            "manager_row" => return Ok(crate::manager::row(&event)),
            "track" => {
                let session = event["session"].as_u64().ok_or("missing_session")?;
                let window = Window {
                    id: event["id"].as_u64().ok_or("missing_window")?,
                    generation: event["generation"].as_u64().ok_or("missing_generation")?,
                };
                let connection = match self.connection {
                    Some(value) => value,
                    None => {
                        let value = self
                            .controller
                            .connected()
                            .ok_or("connection_unavailable")?;
                        self.connection = Some(value);
                        value
                    }
                };
                self.sessions.insert(session, window);
                actions.extend(
                    self.controller
                        .catalog(connection, self.sessions.values().copied()),
                );
                if let Some(action) = self.controller.open(connection, window) {
                    actions.push(action);
                }
            }
            "forget" => {
                let session = event["session"].as_u64().ok_or("missing_session")?;
                self.sessions.remove(&session);
                if let Some(connection) = self.connection {
                    actions.extend(
                        self.controller
                            .catalog(connection, self.sessions.values().copied()),
                    );
                }
            }
            "disconnect" => {
                if let Some(connection) = self.connection.take() {
                    actions.extend(self.controller.disconnected(connection));
                }
                self.sessions.clear();
            }
            "quality" => {
                let preset = event["preset"].as_u64().ok_or("invalid_quality")?;
                let quality = quality(preset).ok_or("invalid_quality")?;
                self.preset = preset;
                actions = self.controller.quality(quality);
            }
            "profile" => {
                return Ok(kokoda_protocol::quality_profile(
                    quality(event["preset"].as_u64().ok_or("invalid_quality")?)
                        .ok_or("invalid_quality")?,
                ))
            }
            _ => return Err("unknown_action"),
        }
        Ok(
            json!({"preset":self.preset,"actions":actions.into_iter().map(|a| {
            let (kind,window,profile)=match a {
                Action::Open(w,q)=>("open",w,Some(kokoda_protocol::quality_profile(q))),
                Action::ApplyQuality(w,q)=>("quality",w,Some(kokoda_protocol::quality_profile(q))),
                Action::Focus(w)=>("focus",w,None), Action::Close(w)=>("close",w,None),
            };
            json!({"type":kind,"id":window.id,"generation":window.generation,"profile":profile})
        }).collect::<Vec<_>>()}),
        )
    }
}
fn quality(preset: u64) -> Option<Quality> {
    Some(match preset {
        0 => Quality::Original,
        1 => Quality::Smooth,
        2 => Quality::Clear,
        3 => Quality::HighFrameRate,
        _ => return None,
    })
}

#[no_mangle]
pub extern "C" fn kokoda_client_new() -> *mut c_void {
    Box::into_raw(Box::new(Bridge::new())).cast()
}
/// # Safety
/// `client` must be a live handle from kokoda_client_new, used on one thread, freed once.
#[no_mangle]
pub unsafe extern "C" fn kokoda_client_free(client: *mut c_void) {
    if !client.is_null() {
        drop(Box::from_raw(client.cast::<Bridge>()));
    }
}
/// # Safety
/// Handle must be live and exclusively accessed. `input` must be NUL-terminated UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kokoda_client_dispatch(
    client: *mut c_void,
    input: *const c_char,
) -> *mut c_char {
    if client.is_null() || input.is_null() {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = CStr::from_ptr(input).to_bytes();
        if bytes.len() > 64 * 1024 {
            return Err("action_too_large");
        }
        let value = serde_json::from_slice(bytes).map_err(|_| "invalid_json")?;
        (&mut *client.cast::<Bridge>()).dispatch(value)
    }));
    let value = match result {
        Ok(Ok(v)) => v,
        Ok(Err(code)) => json!({"error":code}),
        Err(_) => json!({"error":"internal_error"}),
    };
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}
/// # Safety
/// `text` must be a result of kokoda_client_dispatch and freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn kokoda_client_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_runner_reuses_global_quality_and_clears_disconnected_viewers() {
        let mut bridge = Bridge::new();
        for session in [1, 2] {
            let result = bridge
                .dispatch(json!({"type":"track","session":session,"id":session*10,"generation":1}))
                .unwrap();
            assert_eq!(result["actions"][0]["profile"]["fps"], 30);
        }
        assert_eq!(
            bridge
                .dispatch(json!({"type":"quality","preset":3}))
                .unwrap()["actions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        bridge.dispatch(json!({"type":"disconnect"})).unwrap();
        let result = bridge
            .dispatch(json!({"type":"track","session":3,"id":30,"generation":2}))
            .unwrap();
        assert_eq!(result["actions"][0]["profile"]["fps"], 60);
        assert!(bridge
            .dispatch(json!({"type":"quality","preset":99}))
            .is_err());
        assert_eq!(bridge.preset, 3);
    }
    #[test]
    fn c_abi_returns_owned_json_and_handles_invalid_events() {
        unsafe {
            let client = kokoda_client_new();
            for (input, expected) in [
                (r#"{"type":"profile","preset":2}"#, "codec"),
                ("{", "error"),
            ] {
                let input = CString::new(input).unwrap();
                let output = kokoda_client_dispatch(client, input.as_ptr());
                let value: Value =
                    serde_json::from_slice(CStr::from_ptr(output).to_bytes()).unwrap();
                assert!(value.get(expected).is_some());
                kokoda_client_string_free(output);
            }
            kokoda_client_free(client);
        }
    }
}
