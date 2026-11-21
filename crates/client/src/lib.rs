//! Shared controller state. Only the authenticated transport supplies catalogs.
pub use kokoda_app_model::Quality;
use kokoda_app_model::Window;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connection(u64);

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Open(Window, Quality),
    Focus(Window),
    Close(Window),
    ApplyQuality(Window, Quality),
}

#[derive(Default)]
pub struct Controller {
    epoch: u64,
    connection: Option<Connection>,
    catalog: BTreeSet<Window>,
    viewers: BTreeSet<Window>,
    quality: Quality,
}

impl Controller {
    /// Called after admission. A second connection requires a disconnect first.
    pub fn connected(&mut self) -> Option<Connection> {
        if self.connection.is_some() {
            return None;
        }
        self.epoch = self.epoch.checked_add(1)?;
        self.connection = Some(Connection(self.epoch));
        self.connection
    }

    pub fn catalog(
        &mut self,
        source: Connection,
        windows: impl IntoIterator<Item = Window>,
    ) -> Vec<Action> {
        if self.connection != Some(source) {
            return vec![];
        }
        self.catalog = windows.into_iter().collect();
        let removed = self
            .viewers
            .difference(&self.catalog)
            .copied()
            .map(Action::Close)
            .collect();
        self.viewers.retain(|w| self.catalog.contains(w));
        removed
    }

    pub fn open(&mut self, source: Connection, window: Window) -> Option<Action> {
        if self.connection != Some(source) || !self.catalog.contains(&window) {
            return None;
        }
        Some(if self.viewers.insert(window) {
            Action::Open(window, self.quality)
        } else {
            Action::Focus(window)
        })
    }

    pub fn close(&mut self, source: Connection, window: Window) -> Option<Action> {
        (self.connection == Some(source) && self.viewers.remove(&window))
            .then_some(Action::Close(window))
    }

    pub fn quality(&mut self, quality: Quality) -> Vec<Action> {
        self.quality = quality;
        self.viewers
            .iter()
            .copied()
            .map(|w| Action::ApplyQuality(w, quality))
            .collect()
    }

    pub fn disconnected(&mut self, source: Connection) -> Vec<Action> {
        if self.connection != Some(source) {
            return vec![];
        }
        self.connection = None;
        self.catalog.clear();
        std::mem::take(&mut self.viewers)
            .into_iter()
            .map(Action::Close)
            .collect()
    }
}

pub mod ffi;
