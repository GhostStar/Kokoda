use super::{input::Target, live_capture::EncodedFrame, windows};
use kokoda_app_model::AppWindow;
use kokoda_transport::tls::Result;
use std::collections::{BTreeMap, HashSet};

const MAX_WINDOWS: usize = 16;
struct Member {
    window: AppWindow,
    token: u64,
    epoch: u64,
    output_sequence: u64,
    flow: crate::flow::Flow,
    viewport: Option<(u32, u32)>,
    target: Target,
    capture: Option<super::video_worker::VideoWorker>,
}

pub struct Group {
    sharing: std::sync::Arc<super::sharing::Sharing>,
    root: u64,
    root_identity: Target,
    root_snapshot: AppWindow,
    application: bool,
    alpha_video: bool,
    playback_feedback: bool,
    members: BTreeMap<u64, Member>,
    order: Vec<u64>,
    active_input: Option<u64>,
    next_token: u64,
    truncated: bool,
    video: Option<super::video::VideoOptions>,
    viewport: Option<(u32, u32)>,
    last_sync: std::time::Instant,
    frame_cursor: usize,
    announced: BTreeMap<u64, u64>,
    manages_windows: bool,
    family: Option<kokoda_app_model::window_management::ProcessFamily>,
    family_sync: std::time::Instant,
}

impl Group {
    pub fn new(
        id: u64,
        sharing: std::sync::Arc<super::sharing::Sharing>,
        application: bool,
        video: Option<super::video::VideoOptions>,
        alpha_video: bool,
        playback_feedback: bool,
        manages_windows: bool,
    ) -> Result<Self> {
        let root = sharing
            .catalog()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or("Window unavailable")?;
        let root_identity = Target::new(id)?;
        if manages_windows {
            root_identity.validate_identity()?
        } else {
            root_identity.validate()?
        }
        let family = if application {
            windows::process_details(root.process_id)
                .ok()
                .map(|(executable, started)| {
                    kokoda_app_model::window_management::ProcessFamily::new(
                        &kokoda_app_model::window_management::ProcessFact {
                            pid: root.process_id,
                            parent: 0,
                            started,
                            executable,
                        },
                    )
                })
        } else {
            None
        };
        let mut group = Self {
            manages_windows,
            family,
            family_sync: std::time::Instant::now() - std::time::Duration::from_secs(1),
            sharing,
            root: id,
            root_identity,
            root_snapshot: root,
            application,
            alpha_video,
            playback_feedback,
            members: BTreeMap::new(),
            order: Vec::new(),
            active_input: None,
            next_token: 0,
            truncated: false,
            video,
            viewport: None,
            last_sync: std::time::Instant::now() - std::time::Duration::from_secs(1),
            frame_cursor: 0,
            announced: BTreeMap::new(),
        };
        group.sync()?;
        Ok(group)
    }

    pub fn sync(&mut self) -> Result<()> {
        if self.root_identity.validate_identity().is_err() {
            if !self.manages_windows || !self.application || self.sharing.restricted() {
                return Err("Selected window closed".into());
            }
            let next = self
                .members
                .values()
                .find(|m| {
                    m.target.validate_identity().is_ok()
                        && kokoda_platform_windows::window_management::presentation(m.window.id)
                            .role
                            == kokoda_app_model::window_management::WindowRole::Independent
                })
                .ok_or("Application windows closed")?;
            self.root = next.window.id;
            self.root_snapshot = next.window.clone();
            self.root_identity = Target::new(self.root)?;
            self.last_sync = std::time::Instant::now() - std::time::Duration::from_secs(1);
        }
        if self.last_sync.elapsed() < std::time::Duration::from_millis(100) {
            return Ok(());
        }
        self.last_sync = std::time::Instant::now();
        let mut all = self.sharing.catalog()?;
        if self.sharing.restricted() && !all.iter().any(|w| w.id == self.root) {
            self.members.clear();
            self.order.clear();
            return Err("Window sharing permission revoked".into());
        }
        // A hidden root remains the identity/coordinate anchor; it is not captured.
        if !all.iter().any(|w| w.id == self.root) {
            all.push(self.root_snapshot.clone());
        }
        if self.family_sync.elapsed() >= std::time::Duration::from_millis(500) {
            if let Some(family) = self.family.as_mut() {
                let facts =
                    kokoda_platform_windows::window_management::process_facts(&family.seeds())
                        .unwrap_or_default();
                family.update(&facts);
            }
            self.family_sync = std::time::Instant::now();
        }
        let scope = if self.application {
            // Revalidate live process identity between tree refreshes, including
            // new HWNDs after PID reuse. A cached PID alone never grants membership.
            let mut processes = self
                .family
                .as_ref()
                .map(|f| {
                    f.pids()
                        .into_iter()
                        .filter(|pid| {
                            windows::process_details(*pid)
                                .is_ok_and(|(_, started)| f.matches(*pid, started))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            processes.push(self.root_snapshot.process_id);
            kokoda_app_model::application_windows(&all, self.root, &processes)?
        } else {
            all.iter().filter(|w| w.id == self.root).collect()
        };
        let mut visible: Vec<AppWindow> = scope
            .into_iter()
            .filter(|w| self.manages_windows || !w.minimized)
            .filter(|w| windows::visible(w.id))
            .cloned()
            .collect();
        for window in &mut visible {
            if window.minimized {
                if let Some(old) = self.members.get(&window.id) {
                    window.bounds = old.window.bounds.clone()
                }
            } else {
                windows::capture_bounds(window);
            }
        }
        visible.retain(|w| w.bounds.right > w.bounds.left && w.bounds.bottom > w.bounds.top);
        self.order = visible.iter().map(|w| w.id).collect(); // EnumWindows is top to bottom.
        if let Some(root) = visible.iter().find(|w| w.id == self.root) {
            self.root_snapshot = root.clone();
        }
        self.truncated = visible.len() > MAX_WINDOWS;
        // Always reserve the root, then follow z-order for the auxiliary windows.
        visible.sort_by_key(|w| w.id != self.root);
        visible.truncate(MAX_WINDOWS);
        let allowed: HashSet<u64> = visible.iter().map(|w| w.id).collect();
        self.members
            .retain(|id, m| allowed.contains(id) && m.target.validate_identity().is_ok());
        self.order.retain(|id| allowed.contains(id));
        for window in visible {
            if let Some(member) = self.members.get_mut(&window.id) {
                if member.window.minimized != window.minimized {
                    member.capture = None;
                    member.epoch += 1;
                    let _ = member.target.release();
                }
                member.window = window;
            } else if let Ok(target) = Target::new(window.id) {
                self.next_token += 1;
                self.members.insert(
                    window.id,
                    Member {
                        window,
                        token: self.next_token,
                        epoch: 1,
                        output_sequence: 0,
                        flow: {
                            let mut flow = crate::flow::Flow::default();
                            flow.set_ceiling(
                                self.video.map(|v| crate::flow::Rate {
                                    fps: v.fps,
                                    bitrate: v.bitrate,
                                }),
                                std::time::Instant::now(),
                            );
                            flow
                        },
                        viewport: None,
                        target,
                        capture: None,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn snapshot(&mut self) -> Result<serde_json::Value> {
        if self.sharing.restricted() {
            self.last_sync = std::time::Instant::now() - std::time::Duration::from_secs(1);
        }
        self.sync()?;
        // Geometry refresh is independent of the expensive catalog/ownership
        // discovery cadence. Native dragging must not be quantized to 100 ms.
        for member in self.members.values_mut() {
            if member.target.validate().is_ok() {
                windows::capture_bounds(&mut member.window);
            }
        }
        if let Some(root) = self.members.get(&self.root) {
            self.root_snapshot.bounds = root.window.bounds.clone();
        }
        self.announced = self.members.iter().map(|(id, m)| (*id, m.token)).collect();
        let members: Vec<_> = self
            .order
            .iter()
            .filter_map(|id| self.members.get(id))
            .map(|m| {
                let mut value = serde_json::to_value(&m.window).unwrap();
                value["token"] = m.token.into();
                if self.manages_windows {
                    let mut presentation =
                        kokoda_platform_windows::window_management::presentation(m.window.id);
                    presentation.owner = presentation
                        .owner
                        .filter(|id| self.members.contains_key(id));
                    value["presentation"] = serde_json::to_value(presentation).unwrap();
                }
                value
            })
            .collect();
        Ok(
            serde_json::json!({"type":"window_group", "root_id":self.root,
            "root_bounds": self.root_snapshot.bounds, "windows":members,
            "foreground":self.members.contains_key(&windows::foreground()).then(windows::foreground), "truncated":self.truncated}),
        )
    }

    pub fn contains_member(&self, id: u64, token: Option<u64>) -> bool {
        self.sharing.permits(self.root, id)
            && self.members.get(&id).is_some_and(|m| {
                token.is_none_or(|t| t == m.token) && m.target.validate_identity().is_ok()
            })
    }
    pub fn contains_visible(&self, id: u64, token: Option<u64>) -> bool {
        self.sharing.permits(self.root, id)
            && self
                .members
                .get(&id)
                .is_some_and(|m| token.is_none_or(|t| t == m.token) && m.target.validate().is_ok())
    }

    pub fn poll_clipboard(
        &mut self,
        id: u64,
        token: u64,
        before: u32,
        clipboard: &super::clipboard::Clipboard,
    ) -> Result<super::clipboard::Poll> {
        self.sync()?;
        self.member(id, Some(token))?;
        let pids: Vec<u32> = self
            .members
            .values()
            .filter(|m| m.target.validate().is_ok())
            .map(|m| m.window.process_id)
            .collect();
        clipboard.poll(before, &pids)
    }

    pub fn release_inputs(&mut self) {
        for member in self.members.values_mut() {
            let _ = member.target.release();
        }
        self.active_input = None;
    }

    pub fn set_video(
        &mut self,
        id: u64,
        token: u64,
        video: Option<super::video::VideoOptions>,
    ) -> Result<u64> {
        self.member(id, Some(token))?;
        if id != self.root {
            return Err("Video settings require the root window".into());
        }
        if self.video != video {
            let mode_changed = self.video.is_some() != video.is_some();
            for member in self.members.values_mut() {
                member.epoch += 1;
                member.flow.set_ceiling(
                    video.map(|v| crate::flow::Rate {
                        fps: v.fps,
                        bitrate: v.bitrate,
                    }),
                    std::time::Instant::now(),
                );
                if mode_changed {
                    member.capture = None;
                }
            }
            self.video = video;
        }
        Ok(self.members[&id].epoch)
    }

    pub fn window_action(
        &mut self,
        id: u64,
        token: u64,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        if !self.manages_windows || self.sharing.restricted() {
            return Err("Window actions are not authorized".into());
        }
        self.sync()?;
        if !self.sharing.permits(self.root, id) {
            return Err("Window sharing permission revoked".into());
        }
        let member = self
            .members
            .get(&id)
            .ok_or("Window is not in the selected application")?;
        if member.token != token {
            return Err("Stale window generation".into());
        }
        member.target.validate_identity()?;
        self.release_inputs();
        if request["action"] == "activate" {
            self.members[&id].target.activate()?;
        } else {
            kokoda_platform_windows::window_management::action(id, request)?;
        }
        self.last_sync = std::time::Instant::now() - std::time::Duration::from_secs(1);
        Ok(
            serde_json::json!({"type":"window_action_ok","id":id,"token":token,"action":request["action"],"revision":request["revision"].as_u64().unwrap_or(0),"final":request["final"].as_bool().unwrap_or(false)}),
        )
    }

    pub fn set_viewport(&mut self, id: u64, token: u64, width: u32, height: u32) -> Result<()> {
        self.sync()?;
        self.member(id, Some(token))?;
        if (!self.alpha_video && id != self.root) || self.video.is_none() {
            return Err("Viewport requires an H264 window".into());
        }
        let viewport = match (width, height) {
            (0, 0) => None,
            (1..=8192, 1..=8192) => Some((width, height)),
            _ => return Err("Invalid viewport size".into()),
        };
        self.members.get_mut(&id).unwrap().viewport = viewport;
        if id == self.root {
            self.viewport = viewport;
        }
        Ok(())
    }

    pub fn clipboard(
        &mut self,
        id: u64,
        token: u64,
        action: &str,
        text: Option<&str>,
        image_png: Option<&str>,
        clipboard: &super::clipboard::Clipboard,
    ) -> Result<Option<super::clipboard::Content>> {
        let vk = match action {
            "copy" => 0x43,
            "cut" => 0x58,
            "paste" => 0x56,
            _ => return Err("Unknown clipboard action".into()),
        };
        self.sync()?;
        self.member(id, Some(token))?.target.activate()?;
        self.release_inputs();
        let before = clipboard.sequence();
        if action == "paste" {
            match (text, image_png) {
                (Some(text), None) => clipboard.set(text)?,
                (None, Some(png)) => clipboard.set_png(png)?,
                _ => return Err("Expected exactly one clipboard text or PNG payload".into()),
            }
        }
        use kokoda_app_model::input::InputEvent::Key;
        let result = self.input(
            id,
            Some(token),
            vec![
                Key { vk: 17, down: true },
                Key { vk, down: true },
                Key { vk, down: false },
                Key {
                    vk: 17,
                    down: false,
                },
            ],
        );
        self.release_inputs();
        result?;
        if action == "paste" {
            Ok(None)
        } else {
            clipboard.changed_content(before)
        }
    }

    pub fn title(&self) -> &str {
        &self.root_snapshot.title
    }
    pub fn root(&self) -> u64 {
        self.root
    }

    fn member(&mut self, id: u64, token: Option<u64>) -> Result<&mut Member> {
        let member = self.member_identity(id, token)?;
        member.target.validate()?;
        Ok(member)
    }
    fn member_identity(&mut self, id: u64, token: Option<u64>) -> Result<&mut Member> {
        if !self.sharing.permits(self.root, id) {
            return Err("Window sharing permission revoked".into());
        }
        let member = self
            .members
            .get_mut(&id)
            .ok_or("Window is not in the selected application")?;
        if token.is_some_and(|token| token != member.token) {
            return Err("Stale window generation".into());
        }
        member.target.validate_identity()?;
        Ok(member)
    }

    pub fn frame(
        &mut self,
        id: u64,
        token: Option<u64>,
        after: u64,
    ) -> Result<(u64, Option<(f64, f64)>, Option<EncodedFrame>)> {
        self.sync()?;
        // Ownership does not imply transparency. Negotiated video carries an
        // optional alpha plane measured from the captured pixels.
        let transparent = self.alpha_video || id != self.root;
        let mut video = if self.alpha_video || id == self.root {
            self.video
        } else {
            None
        };
        let viewport = if self.alpha_video {
            self.members.get(&id).and_then(|m| m.viewport)
        } else {
            self.viewport
        };
        let sharing = self.sharing.clone();
        let root = self.root;
        let feedback = self.playback_feedback;
        let member = self.member(id, token)?;
        if let Some(options) = video.as_mut() {
            let b = &member.window.bounds;
            *options = options.for_surface(
                (
                    (b.right - b.left).max(16) as u32,
                    (b.bottom - b.top).max(16) as u32,
                ),
                viewport,
            );
            if member.flow.resize_ceiling(
                crate::flow::Rate {
                    fps: options.fps,
                    bitrate: options.bitrate,
                },
                std::time::Instant::now(),
            ) {
                member.epoch += 1;
            }
        }
        if feedback {
            if let (Some(options), Some(rate)) = (video.as_mut(), member.flow.rate()) {
                options.fps = rate.fps;
                options.bitrate = rate.bitrate;
            }
        }
        if member.capture.is_none() {
            member.capture = Some(super::video_worker::VideoWorker::start(
                id,
                transparent,
                video,
                viewport,
                member.epoch,
                Some(std::sync::Arc::new(move || sharing.permits(root, id))),
            ));
        }
        let mut frame = member
            .capture
            .as_ref()
            .unwrap()
            .frame(viewport, video, member.epoch)?;
        // A queued packet from the old encoder must not undo a live settings
        // change; the new epoch begins with a new encoder's keyframe.
        frame = frame.filter(|f| f.epoch == member.epoch);
        if let Some(frame) = frame.as_mut() {
            member.output_sequence += 1;
            frame.sequence = member.output_sequence;
        }
        let frame = frame.filter(|f| f.sequence > after);
        member.target.validate()?;
        if !self.sharing.permits(self.root, id) {
            return Err("Window sharing permission revoked".into());
        }
        let member = self.members.get(&id).ok_or("Window unavailable")?;
        Ok((
            member.token,
            if self.sharing.restricted() {
                None
            } else {
                member.target.caret()
            },
            frame,
        ))
    }

    pub fn next_frame(&mut self) -> Result<Option<(u64, u64, Option<(f64, f64)>, EncodedFrame)>> {
        self.sync()?;
        let ids: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.members.get(id).is_some_and(|m| {
                    self.announced.get(id) == Some(&m.token)
                        && (!self.playback_feedback || m.flow.ready())
                })
            })
            .collect();
        for _ in 0..ids.len() {
            let id = ids[self.frame_cursor % ids.len()];
            self.frame_cursor = self.frame_cursor.wrapping_add(1);
            // Visibility can change between the cached enumeration and capture.
            // A disappearing popup must not terminate its application's stream.
            if !self.contains_visible(id, None) {
                continue;
            }
            let (token, caret, frame) = match self.frame(id, None, 0) {
                Ok(value) => value,
                Err(_) if id != self.root => {
                    self.members.remove(&id);
                    self.order.retain(|other| *other != id);
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Some(frame) = frame {
                if self.playback_feedback {
                    self.members.get_mut(&id).unwrap().flow.sent(
                        frame.epoch,
                        frame.sequence,
                        std::time::Instant::now(),
                    );
                }
                return Ok(Some((id, token, caret, frame)));
            }
        }
        Ok(None)
    }

    pub fn feedback(
        &mut self,
        id: u64,
        token: u64,
        epoch: u64,
        sequence: u64,
        decode_ms: f64,
        queue_ms: f64,
    ) -> Result<serde_json::Value> {
        if !self.playback_feedback {
            return Err("Playback feedback not negotiated".into());
        }
        self.sync()?;
        let member = self.member_identity(id, Some(token))?;
        if member.flow.consumed(
            epoch,
            sequence,
            member.epoch,
            decode_ms,
            queue_ms,
            std::time::Instant::now(),
        )? {
            member.epoch += 1;
        }
        let rate = member.flow.rate();
        let ceiling = member.flow.ceiling();
        Ok(
            serde_json::json!({"type":"video_feedback_ok","id":id,"token":token,"fps":rate.map(|r|r.fps),"bitrate":rate.map(|r|r.bitrate),"bitrate_ceiling":ceiling.map(|r|r.bitrate),"adaptation":member.flow.reason()}),
        )
    }

    pub fn input(
        &mut self,
        id: u64,
        token: Option<u64>,
        events: Vec<kokoda_app_model::input::InputEvent>,
    ) -> Result<()> {
        self.sync()?;
        // Validate membership before releasing or routing any state.
        self.member_identity(id, token)?;
        let only_release = events
            .iter()
            .all(|e| matches!(e, kokoda_app_model::input::InputEvent::Release));
        if !only_release && self.active_input != Some(id) {
            if let Some(old) = self.active_input {
                if let Some(member) = self.members.get_mut(&old) {
                    member.target.release()?;
                }
            }
            self.active_input = Some(id);
        }
        let member = if only_release {
            self.member_identity(id, token)?
        } else {
            self.member(id, token)?
        };
        for event in events {
            if let Err(error) = member.target.apply(event) {
                let _ = member.target.release();
                return Err(error);
            }
        }
        Ok(())
    }
}
