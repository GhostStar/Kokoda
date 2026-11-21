use kokoda_host::sharing::{Mode, Policy};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub all_windows: bool,
    pub discoverable: bool,
    pub port: Option<u16>,
    pub policy: Policy,
}
impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.port == Some(0) || self.policy.mode != Mode::Selected {
            return Err("invalid_host_settings");
        }
        self.policy.validate()
    }
    pub fn arguments(&self, directory: &Path) -> Vec<String> {
        let mut args = vec![
            "--serve-password".into(),
            format!("0.0.0.0:{}", self.port.unwrap_or(47831)),
            directory
                .join("password.dat")
                .to_string_lossy()
                .into_owned(),
        ];
        if self.all_windows {
            args.push("--allow-all-windows".into());
        } else {
            args.extend([
                "--sharing-policy".into(),
                directory
                    .join("sharing.json")
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
        args
    }
}
pub fn load(directory: &Path) -> Result<Settings, Box<dyn std::error::Error + Send + Sync>> {
    let path = directory.join("settings.json");
    if !path.exists() {
        return Ok(Settings::default());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("Host settings exceed 64 KiB".into());
    }
    let settings: Settings = serde_json::from_slice(&bytes)?;
    settings.validate()?;
    Ok(settings)
}
pub fn save(
    directory: &Path,
    settings: &Settings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    settings.validate()?;
    std::fs::create_dir_all(directory)?;
    let files = [
        ("settings.json", serde_json::to_vec_pretty(settings)?),
        ("sharing.json", serde_json::to_vec_pretty(&settings.policy)?),
    ];
    if files.iter().any(|(_, data)| data.len() > 65536) {
        return Err("Host settings exceed 64 KiB".into());
    }
    for (name, data) in files {
        let target = directory.join(name);
        let temporary = target.with_extension("tmp");
        std::fs::write(&temporary, data)?;
        std::fs::rename(temporary, target)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn default_is_empty_selected_and_all_mode_requires_explicit_setting() {
        let mut settings = Settings::default();
        let base = Path::new("host data");
        assert!(settings.policy.rules.is_empty());
        assert!(settings
            .arguments(base)
            .contains(&"--sharing-policy".into()));
        settings.all_windows = true;
        let args = settings.arguments(base);
        assert_eq!(args.len(), 4);
        assert_eq!(args[3], "--allow-all-windows");
        settings.port = Some(0);
        assert!(settings.validate().is_err());
        settings.port = Some(47831);
        settings.policy.mode = Mode::All;
        assert!(settings.validate().is_err());
    }
    #[test]
    fn settings_roundtrip_rejects_oversize_and_invalid_config() {
        let path = std::env::temp_dir().join(format!(
            "kokoda-gui-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        assert!(!load(&path).unwrap().all_windows);
        save(&path, &Settings::default()).unwrap();
        let settings = Settings {
            all_windows: true,
            ..Default::default()
        };
        save(&path, &settings).unwrap();
        assert!(load(&path).unwrap().all_windows);
        let oversized = Settings {
            policy: Policy {
                rules: vec![
                    kokoda_host::sharing::Rule {
                        executable: None,
                        title_contains: Some("x".repeat(1024))
                    };
                    128
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(save(&path, &oversized).is_err());
        assert!(load(&path).unwrap().all_windows);
        std::fs::write(path.join("settings.json"), b"{invalid").unwrap();
        assert!(load(&path).is_err());
    }
}
