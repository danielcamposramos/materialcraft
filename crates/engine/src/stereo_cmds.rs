//! Session-only choices for a scene rendered through two coherent cameras.
use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, always, bad},
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Settings {
    pub enabled: bool,
    pub resolution: Option<[u32; 2]>,
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "view.stereo",
        "Stereo view",
        [],
        None,
        "{enabled?: bool, resolution?: [width, height] | null}; omitted fields stay; null uses the 3D view size",
        always,
        settings
    )]
}

fn settings(s: &mut Session, p: &Value) -> Result<Value> {
    let mut next = s.stereo;
    if let Some(value) = p.get("enabled") {
        next.enabled = value.as_bool().ok_or_else(|| bad("view.stereo", "enabled must be a boolean"))?;
    }
    if let Some(value) = p.get("resolution") {
        next.resolution = if value.is_null() {
            None
        } else {
            let invalid = || bad("view.stereo", "resolution must be null or [width, height], each between 1 and 4096");
            let values = value.as_array().filter(|v| v.len() == 2).ok_or_else(invalid)?;
            let edge = |index: usize| values.get(index).and_then(Value::as_u64).filter(|v| (1..=4096).contains(v)).map(|v| v as u32).ok_or_else(invalid);
            Some([edge(0)?, edge(1)?])
        };
    }
    s.stereo = next;
    Ok(json!(next))
}
