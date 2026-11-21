use crate::sharing::{Mode, Policy, ProcessInstance, WindowFacts};
use kokoda_app_model::AppWindow;
use kokoda_transport::tls::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

/// Immutable for the server lifetime. Restarting with a new policy drops every
/// connection, worker and resume ticket; no client command can change this policy.
#[derive(Clone, Default)]
pub struct Sharing {
    policy: Policy,
}
impl Sharing {
    pub fn all_development() -> Self {
        Self {
            policy: Policy {
                mode: Mode::All,
                rules: vec![],
            },
        }
    }
    pub fn restricted(&self) -> bool {
        self.policy.mode == Mode::Selected
    }
    pub fn load(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err("Sharing policy exceeds 64 KiB".into());
        }
        let mut policy: Policy =
            serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
                .map_err(|_| "Invalid sharing policy JSON")?;
        policy.validate()?;
        if policy.mode != Mode::Selected {
            return Err(
                "Use --allow-all-windows explicitly for unrestricted development sharing".into(),
            );
        }
        for rule in &mut policy.rules {
            if let Some(path) = rule.executable.as_mut() {
                if !Path::new(path).is_absolute() {
                    return Err("Executable rule requires an absolute path".into());
                }
                *path = std::fs::canonicalize(&path)?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "Executable path is not Unicode")?;
            }
        }
        Ok(Self { policy })
    }
    pub fn catalog(&self) -> Result<Vec<AppWindow>> {
        let mut windows = super::windows::enumerate()?;
        if self.restricted() {
            let mut processes = BTreeMap::new();
            for window in &windows {
                processes
                    .entry(window.process_id)
                    .or_insert_with(|| super::windows::process_details(window.process_id).ok());
            }
            let facts: Vec<_> = windows
                .iter()
                .filter_map(|window| {
                    let (executable, started) = processes.get(&window.process_id)?.as_ref()?;
                    Some(WindowFacts {
                        id: window.id,
                        owner: window.owner,
                        process: ProcessInstance {
                            pid: window.process_id,
                            started: *started,
                        },
                        executable: executable.clone(),
                        title: window.title.clone(),
                    })
                })
                .collect();
            let allowed = self.policy.allowed(&facts);
            windows.retain(|w| allowed.contains(&w.id));
            // An allowed window can itself be owned by a private window.
            for window in &mut windows {
                window.owner = window.owner.filter(|id| allowed.contains(id));
            }
        }
        Ok(windows)
    }
    pub fn permits(&self, root: u64, id: u64) -> bool {
        if !self.restricted() {
            return true;
        }
        let mut ids = BTreeSet::new();
        let mut pending = vec![root, id];
        let mut windows = Vec::new();
        while let Some(id) = pending.pop() {
            if !ids.insert(id) {
                continue;
            }
            if ids.len() > 128 {
                return false;
            }
            let Some(window) = super::windows::inspect(id) else {
                continue;
            };
            if let Some(owner) = window.owner {
                pending.push(owner);
            }
            windows.push(window);
        }
        let facts: Vec<_> = windows
            .iter()
            .filter_map(|w| {
                let (executable, started) = super::windows::process_details(w.process_id).ok()?;
                Some(WindowFacts {
                    id: w.id,
                    owner: w.owner,
                    process: ProcessInstance {
                        pid: w.process_id,
                        started,
                    },
                    executable,
                    title: w.title.clone(),
                })
            })
            .collect();
        let allowed = self.policy.allowed(&facts);
        allowed.contains(&root) && allowed.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_loader_rejects_widening_and_bad_paths_and_accepts_bom() {
        let file = std::env::temp_dir().join(format!(
            "kokoda-policy-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(file.clone());
        for text in [
            r#"{"mode":"all"}"#,
            r#"{"rules":[{}]}"#,
            r#"{"rules":[{"executable":"relative.exe"}]}"#,
            r#"{"rules":[],"allow_all":true}"#,
            "{",
        ] {
            std::fs::write(&file, text).unwrap();
            assert!(Sharing::load(&file).is_err());
        }
        std::fs::write(&file, vec![b' '; 65537]).unwrap();
        assert!(Sharing::load(&file).is_err());
        let executable = std::env::current_exe().unwrap();
        let payload =
            serde_json::json!({"rules":[{"executable":executable,"title_contains":"Fixture"}]})
                .to_string();
        std::fs::write(
            &file,
            [&[0xef, 0xbb, 0xbf][..], payload.as_bytes()].concat(),
        )
        .unwrap();
        let loaded = Sharing::load(&file).unwrap();
        assert!(loaded.restricted());
        assert_eq!(
            loaded.policy.rules[0].executable.as_ref().unwrap(),
            std::fs::canonicalize(executable).unwrap().to_str().unwrap()
        );
    }
}
