//! Host-owned sharing policy. Process/window facts come from native adapters,
//! never from the requesting client. Membership is recomputed from fresh facts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Selected,
    All,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub executable: Option<String>,
    pub title_contains: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInstance {
    pub pid: u32,
    pub started: u64,
}
pub struct WindowFacts {
    pub id: u64,
    pub owner: Option<u64>,
    pub process: ProcessInstance,
    /// Canonical executable path supplied by the host adapter.
    pub executable: String,
    pub title: String,
}
fn normalized_title(value: &str) -> String {
    value
        .nfc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}
impl Policy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.rules.len() > 128 {
            return Err("too_many_sharing_rules");
        }
        if self.mode == Mode::All && !self.rules.is_empty() {
            return Err("all_mode_has_rules");
        }
        for rule in &self.rules {
            if rule.executable.is_none() && rule.title_contains.is_none() {
                return Err("empty_sharing_rule");
            }
            for value in [&rule.executable, &rule.title_contains]
                .into_iter()
                .flatten()
            {
                if value.trim().is_empty()
                    || value.len() > 4096
                    || value.chars().any(char::is_control)
                {
                    return Err("invalid_sharing_condition");
                }
            }
        }
        Ok(())
    }
    pub fn allowed(&self, windows: &[WindowFacts]) -> BTreeSet<u64> {
        // Invalid policy must never widen access, even when called outside the loader.
        if self.validate().is_err() {
            return BTreeSet::new();
        }
        let mut allowed: BTreeSet<_> = windows
            .iter()
            .filter(|window| {
                let title = normalized_title(&window.title);
                self.mode == Mode::All
                    || self.rules.iter().any(|rule| {
                        rule.executable
                            .as_ref()
                            .is_none_or(|path| path == &window.executable)
                            && rule
                                .title_contains
                                .as_ref()
                                .is_none_or(|needle| title.contains(&normalized_title(needle)))
                    })
            })
            .map(|window| window.id)
            .collect();
        loop {
            let before = allowed.len();
            for window in windows {
                if let Some(owner) = window
                    .owner
                    .and_then(|id| windows.iter().find(|w| w.id == id))
                {
                    if allowed.contains(&owner.id) && owner.process == window.process {
                        allowed.insert(window.id);
                    }
                }
            }
            if before == allowed.len() {
                break;
            }
        }
        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(id: u64, title: &str, owner: Option<u64>, pid: u32, started: u64) -> WindowFacts {
        WindowFacts {
            id,
            title: title.into(),
            owner,
            process: ProcessInstance { pid, started },
            executable: "resolved/app.exe".into(),
        }
    }
    fn policy(rules: &str) -> Policy {
        serde_json::from_str(rules).unwrap()
    }
    #[test]
    fn empty_unknown_or_invalid_configuration_never_shares_everything() {
        let windows = [window(1, "secret", None, 1, 1)];
        assert!(Policy::default().allowed(&windows).is_empty());
        assert!(serde_json::from_str::<Policy>(r#"{"rules":[],"rule":{}}"#).is_err());
        for json in [
            r#"{"rules":[{}]}"#,
            r#"{"rules":[{"title_contains":" "}]}"#,
            r#"{"mode":"all","rules":[{}]}"#,
        ] {
            let p = policy(json);
            assert!(p.validate().is_err());
            assert!(p.allowed(&windows).is_empty());
        }
        assert_eq!(
            policy(r#"{"mode":"all"}"#).allowed(&windows),
            BTreeSet::from([1])
        );
    }
    #[test]
    fn conditions_are_anded_rules_are_ored_and_titles_use_canonical_unicode() {
        let mut windows = vec![
            window(1, "Cafe\u{301} PRIVATE", None, 1, 1),
            window(2, "café private", None, 2, 2),
            window(3, "safe", None, 2, 2),
        ];
        windows[1].executable = "other/app.exe".into();
        let p = policy(
            r#"{"rules":[{"executable":"resolved/app.exe","title_contains":"CAFÉ"},{"title_contains":"safe"}]}"#,
        );
        assert_eq!(p.allowed(&windows), BTreeSet::from([1, 3]));
    }
    #[test]
    fn only_verified_owner_chains_inherit_not_siblings_or_reused_process_ids() {
        let windows = vec![
            window(1, "allowed", None, 1, 10),
            window(2, "dialog", Some(1), 1, 10),
            window(3, "nested", Some(2), 1, 10),
            window(4, "sibling", None, 1, 10),
            window(5, "foreign", Some(1), 2, 10),
            window(6, "reused pid", Some(1), 1, 11),
            window(7, "cycle", Some(8), 1, 10),
            window(8, "cycle", Some(7), 1, 10),
        ];
        let p = policy(r#"{"rules":[{"title_contains":"allowed"}]}"#);
        assert_eq!(p.allowed(&windows), BTreeSet::from([1, 2, 3]));
    }
    #[test]
    fn title_or_owner_change_revokes_inherited_access() {
        let mut windows = vec![
            window(1, "allowed", None, 1, 1),
            window(2, "dialog", Some(1), 1, 1),
        ];
        let p = policy(r#"{"rules":[{"title_contains":"allowed"}]}"#);
        windows[1].owner = None;
        assert_eq!(p.allowed(&windows), BTreeSet::from([1]));
        windows[1].owner = Some(1);
        windows[0].title = "private".into();
        assert!(p.allowed(&windows).is_empty());
    }
}
