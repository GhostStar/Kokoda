use std::{collections::HashSet, io};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppWindow {
    pub id: u64,
    pub process_id: u32,
    pub thread_id: u32,
    pub owner: Option<u64>,
    pub title: String,
    pub bounds: WindowBounds,
    pub minimized: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Includes the selected window, its owned dialogs, and explicitly added processes.
/// Process membership is opt-in: a process tree alone does not establish app ownership.
pub fn application_windows<'a>(
    windows: &'a [AppWindow],
    selected: u64,
    additional_processes: &[u32],
) -> io::Result<Vec<&'a AppWindow>> {
    if !windows.iter().any(|window| window.id == selected) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Selected window is no longer available",
        ));
    }
    let mut included: HashSet<u64> = windows
        .iter()
        .filter(|window| window.id == selected || additional_processes.contains(&window.process_id))
        .map(|window| window.id)
        .collect();
    loop {
        let previous = included.len();
        for window in windows {
            if window
                .owner
                .map_or(false, |owner| included.contains(&owner))
            {
                included.insert(window.id);
            }
        }
        if included.len() == previous {
            break;
        }
    }
    Ok(windows
        .iter()
        .filter(|window| included.contains(&window.id))
        .collect())
}

/// Explicit application mode includes independent windows of the selected UI process.
/// Owned windows may cross processes; unrelated executable-name matches are never included.
pub fn application_scope_windows(
    windows: &[AppWindow],
    selected: u64,
) -> io::Result<Vec<&AppWindow>> {
    let root = windows
        .iter()
        .find(|w| w.id == selected)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Selected window unavailable"))?;
    application_windows(windows, selected, &[root.process_id])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, process_id: u32, owner: Option<u64>) -> AppWindow {
        AppWindow {
            id,
            process_id,
            owner,
            thread_id: 1,
            title: String::new(),
            bounds: WindowBounds {
                left: 0,
                top: 0,
                right: 100,
                bottom: 100,
            },
            minimized: false,
        }
    }

    #[test]
    fn includes_owned_dialogs_and_explicit_processes() {
        let windows = vec![
            window(3, 20, Some(2)),
            window(2, 10, Some(1)),
            window(1, 10, None),
            window(4, 30, None),
            window(5, 40, None),
            window(6, 10, None),
        ];
        let result = application_windows(&windows, 1, &[30]).unwrap();
        assert_eq!(
            result.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![3, 2, 1, 4]
        );
    }

    #[test]
    fn application_mode_includes_ownerless_cards_but_not_unrelated_processes() {
        let windows = vec![
            window(1, 10, None),
            window(2, 10, None),
            window(3, 20, Some(2)),
            window(4, 30, None),
        ];
        let scope = application_scope_windows(&windows, 1).unwrap();
        assert_eq!(
            scope.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn missing_selection_does_not_expand_to_other_processes() {
        assert!(application_windows(&[window(1, 10, None)], 99, &[10]).is_err());
    }
}
