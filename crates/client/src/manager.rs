//! Stateless manager presentation rules shared by native frontends.
//! Inputs are projections of an authenticated catalog, never authorization grants.
use serde_json::{json, Value};

pub fn row(value: &Value) -> Value {
    let flag = |name: &str| value[name].as_bool().unwrap_or(false);
    let state = if !flag("connected") {
        "offline"
    } else if flag("ending") {
        "ending"
    } else if flag("opening") {
        "opening"
    } else if flag("blocked") {
        "blocked"
    } else if flag("closed") {
        "hidden"
    } else if flag("minimized") && flag("viewing") {
        "minimized"
    } else if flag("viewing") {
        "viewing"
    } else {
        "available"
    };
    let action = match state {
        "available" => "open",
        "viewing" => "focus",
        "hidden" | "minimized" => "restore",
        "blocked" if flag("modal_available") => "modal",
        _ => "none",
    };
    let title = value["title"].as_str().unwrap_or_default().to_lowercase();
    let query = value["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let rank = if query.is_empty() {
        0
    } else if title == query {
        3
    } else if title.starts_with(&query) {
        2
    } else if title.contains(&query)
        || value["application"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&query)
    {
        1
    } else {
        -1
    };
    json!({"state":state,"action":action,"rank":rank})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manager_actions_match_cross_platform_contract() {
        let cases: Value =
            serde_json::from_str(include_str!("../../../tests/contracts/window-manager.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(row(&case["input"]), case["expected"], "{}", case["name"]);
        }
    }
}
