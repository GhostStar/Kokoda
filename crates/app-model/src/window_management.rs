//! Platform-neutral window roles and conservative process-instance membership.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowRole {
    Independent,
    Floating,
    Modal,
    Transient,
}
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct WindowTraits {
    pub owned: bool,
    pub tool: bool,
    pub app: bool,
    pub owner_disabled: bool,
    pub transient: bool,
}
pub fn role(t: WindowTraits) -> WindowRole {
    if t.transient {
        WindowRole::Transient
    } else if t.owned && t.owner_disabled {
        WindowRole::Modal
    } else if t.app {
        WindowRole::Independent
    } else if t.owned || t.tool {
        WindowRole::Floating
    } else {
        WindowRole::Independent
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Presentation {
    pub dpi: u32,
    pub role: WindowRole,
    pub owner: Option<u64>,
    pub enabled: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub can_maximize: bool,
    pub can_close: bool,
    pub can_minimize: bool,
    pub can_resize: bool,
}

#[derive(Clone, Debug)]
pub struct ProcessFact {
    pub pid: u32,
    pub parent: u32,
    pub started: u64,
    pub executable: String,
}
/// Tracks only descendants of the explicitly selected process with the exact
/// executable identity. A common executable name/vendor is never sufficient.
pub struct ProcessFamily {
    executable: String,
    root: (u32, u64),
    known: BTreeMap<u32, u64>,
}
impl ProcessFamily {
    pub fn new(root: &ProcessFact) -> Self {
        Self {
            executable: root.executable.clone(),
            root: (root.pid, root.started),
            known: BTreeMap::from([(root.pid, root.started)]),
        }
    }
    pub fn pids(&self) -> Vec<u32> {
        self.known.keys().copied().collect()
    }
    pub fn seeds(&self) -> Vec<u32> {
        let mut pids = self.pids();
        if !pids.contains(&self.root.0) {
            pids.push(self.root.0)
        }
        pids
    }
    pub fn matches(&self, pid: u32, started: u64) -> bool {
        self.known.get(&pid) == Some(&started)
    }
    pub fn update(&mut self, facts: &[ProcessFact]) {
        if facts
            .iter()
            .any(|p| (p.pid, p.started) == self.root && p.executable == self.executable)
        {
            self.known.insert(self.root.0, self.root.1);
        }
        self.known.retain(|pid, started| {
            facts
                .iter()
                .any(|p| p.pid == *pid && p.started == *started && p.executable == self.executable)
        });
        for _ in 0..16 {
            let before = self.known.len();
            for p in facts {
                if self.known.len() >= 128 {
                    break;
                }
                if p.executable == self.executable
                    && p.pid != p.parent
                    && self.known.get(&p.parent).is_some_and(|s| *s <= p.started)
                {
                    self.known.insert(p.pid, p.started);
                }
            }
            if before == self.known.len() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_match_shared_contract() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/contracts/window-roles.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let traits = serde_json::from_value(case["traits"].clone()).unwrap();
            assert_eq!(serde_json::to_value(role(traits)).unwrap(), case["role"]);
        }
    }
    #[test]
    fn process_family_rejects_siblings_different_binaries_and_pid_reuse() {
        let fact = |pid, parent, started, path: &str| ProcessFact {
            pid,
            parent,
            started,
            executable: path.into(),
        };
        let root = fact(10, 1, 100, "app");
        let mut family = ProcessFamily::new(&root);
        family.update(&[
            root.clone(),
            fact(11, 10, 101, "app"),
            fact(12, 11, 102, "app"),
            fact(20, 1, 103, "app"),
            fact(30, 10, 104, "browser"),
        ]);
        assert_eq!(family.pids(), vec![10, 11, 12]);
        family.update(&[
            fact(10, 1, 200, "app"),
            fact(11, 10, 101, "app"),
            fact(12, 11, 102, "app"),
            fact(13, 10, 201, "app"),
        ]);
        assert_eq!(family.pids(), vec![11, 12]);
        family.update(&[fact(12, 1, 300, "app")]);
        assert!(family.pids().is_empty());
        family.update(&[root.clone()]);
        assert_eq!(family.pids(), vec![10]);
        assert!(!family.matches(10, 200));
        family.update(&[]);
        assert!(family.pids().is_empty());
        family.update(&[root]);
        assert_eq!(family.pids(), vec![10]);
    }
}
