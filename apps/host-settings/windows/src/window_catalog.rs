use kokoda_host::sharing::{Policy, Rule, WindowFacts};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub id: u64,
    pub pid: u32,
    pub started: u64,
    pub title: String,
    pub executable: String,
    pub owner: Option<u64>,
}
impl Sample {
    pub fn same_instance(&self, other: &Self) -> bool {
        self.id == other.id
            && self.pid == other.pid
            && self.started == other.started
            && self.executable == other.executable
    }
    pub fn rule(&self, mode: usize) -> Result<Rule, &'static str> {
        let rule = match mode {
            0 => Rule {
                executable: Some(self.executable.clone()),
                title_contains: Some(self.title.clone()),
            },
            1 => Rule {
                executable: Some(self.executable.clone()),
                title_contains: None,
            },
            2 => Rule {
                executable: None,
                title_contains: Some(self.title.clone()),
            },
            _ => return Err("invalid_match_mode"),
        };
        Policy {
            rules: vec![rule.clone()],
            ..Default::default()
        }
        .validate()?;
        Ok(rule)
    }
    fn facts(&self) -> WindowFacts {
        WindowFacts {
            id: self.id,
            owner: self.owner,
            process: kokoda_host::sharing::ProcessInstance {
                pid: self.pid,
                started: self.started,
            },
            executable: self.executable.clone(),
            title: self.title.clone(),
        }
    }
}
pub fn preview(rule: Rule, samples: &[Sample]) -> Vec<(u64, bool)> {
    let policy = Policy {
        rules: vec![rule],
        ..Default::default()
    };
    let facts: Vec<_> = samples.iter().map(Sample::facts).collect();
    let allowed = policy.allowed(&facts);
    facts
        .iter()
        .filter(|f| allowed.contains(&f.id))
        .map(|f| {
            (
                f.id,
                policy.allowed(std::slice::from_ref(f)).contains(&f.id),
            )
        })
        .collect()
}
#[cfg(windows)]
pub fn enumerate() -> std::io::Result<Vec<Sample>> {
    use kokoda_platform_windows::windows;
    let mut processes = std::collections::BTreeMap::new();
    let mut samples = Vec::new();
    for w in windows::enumerate()?.into_iter().take(2048) {
        if w.process_id == std::process::id() {
            continue;
        }
        if let Some((path, started)) = processes
            .entry(w.process_id)
            .or_insert_with(|| windows::process_details(w.process_id).ok())
            .as_ref()
        {
            samples.push(Sample {
                id: w.id,
                pid: w.process_id,
                started: *started,
                title: w.title,
                executable: path.clone(),
                owner: w.owner,
            });
        }
    }
    samples.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(samples)
}
#[cfg(windows)]
pub fn revalidate(sample: &Sample) -> bool {
    use kokoda_platform_windows::windows;
    let Some(w) = windows::inspect(sample.id) else {
        return false;
    };
    let Ok((path, started)) = windows::process_details(w.process_id) else {
        return false;
    };
    sample.same_instance(&Sample {
        id: w.id,
        pid: w.process_id,
        started,
        title: w.title.clone(),
        executable: path,
        owner: w.owner,
    }) && sample.title == w.title
        && sample.owner == w.owner
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(id: u64, title: &str, owner: Option<u64>) -> Sample {
        Sample {
            id,
            pid: 10,
            started: 20,
            title: title.into(),
            executable: "C:\\Fixture.exe".into(),
            owner,
        }
    }
    #[test]
    fn examples_do_not_silently_expand_to_same_process() {
        let root = sample(1, "Public", None);
        let other = sample(2, "Private", None);
        let dialog = sample(3, "Dialog", Some(1));
        let rows = vec![root.clone(), other, dialog];
        assert_eq!(
            preview(root.rule(0).unwrap(), &rows),
            vec![(1, true), (3, false)]
        );
        assert_eq!(
            preview(root.rule(1).unwrap(), &rows),
            vec![(1, true), (2, true), (3, true)]
        );
        let mut reused = root.clone();
        reused.started += 1;
        assert!(!root.same_instance(&reused));
        assert!(sample(4, "", None).rule(0).is_err());
        assert!(sample(4, "", None).rule(1).is_ok());
        assert!(root.rule(3).is_err());
    }
}
