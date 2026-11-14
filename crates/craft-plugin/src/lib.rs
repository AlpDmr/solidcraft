//! App-independent plugin contracts for Craft applications (desktop & web).
//! Commands and UI declarations are pure data; no egui or CAD kernel dependencies.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod ui_manifest;

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use ui_manifest::{
    DockZone, PluginPanelManifest, RibbonGroupManifest, RibbonItemManifest, RibbonTabManifest,
    UiManifest, UiPlacementManifest, MAX_BUTTONS_PER_GROUP, MAX_PLACEMENTS_PER_PLUGIN, MAX_GROUPS_PER_TAB, MAX_PANELS_PER_PLUGIN,
    MAX_RIBBON_TABS_PER_PLUGIN, UI_CAPABILITY,
};

pub const API: &str = "craft:plugin@0.1.0";
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const MAX_COMPONENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 1024 * 1024;
pub const MAX_MEMORY_MIB: usize = 4096;

pub type Result<T> = std::result::Result<T, HostError>;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostError {
    #[error("permission denied: {permission}")]
    PermissionDenied { permission: String },
    #[error("command {command} failed: {reason}")]
    CommandFailed { command: String, reason: String },
    #[error("plugin limit exceeded: {limit}")]
    LimitExceeded { limit: String },
    #[error("invalid manifest field {field}: {reason}")]
    InvalidManifest { field: String, reason: String },
    #[error("invalid input {field}: {reason}")]
    InvalidInput { field: String, reason: String },
    #[error("plugin trap: {reason}")]
    Trap { reason: String },
    #[error("plugin cancelled")]
    Cancelled,
    #[error("{reason}")]
    UnsupportedCapability { capability: String, reason: String },
}

pub const CAPABILITIES: &[(&str, (u32, u32))] = &[("commands", (0, 1)), ("ui", (0, 1))];

fn capability_version(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    match (parts.next(), parts.next()) {
        (None, _) => Some((major, minor)),
        (Some(patch), None) if patch.parse::<u32>().is_ok() => Some((major, minor)),
        _ => None,
    }
}

pub fn check_capabilities(requested: &[String], app: &str) -> Result<()> {
    if requested.len() > 16 {
        return Err(invalid("capabilities", "at most 16 capabilities"));
    }
    let mut names = BTreeSet::new();
    for capability in requested {
        let parsed = capability
            .split_once('@')
            .and_then(|(name, version)| Some((name, capability_version(version)?)));
        let Some((name, (major, minor))) = parsed.filter(|(name, _)| valid_id(name)) else {
            return Err(invalid(
                "capabilities",
                format!("`{capability}` is not name@major.minor, e.g. \"commands@0.1\""),
            ));
        };
        if !names.insert(name) {
            return Err(invalid("capabilities", format!("`{name}` is listed twice")));
        }
        let unsupported = |reason: String| HostError::UnsupportedCapability {
            capability: capability.clone(),
            reason,
        };
        let Some((_, (have_major, have_minor))) =
            CAPABILITIES.iter().find(|(known, _)| *known == name)
        else {
            return Err(unsupported(format!(
                "needs {name} {major}.{minor}, which this version of {app} does not have; update {app} or the plugin"
            )));
        };
        let (have_major, have_minor) = (*have_major, *have_minor);
        if (major, minor) > (have_major, have_minor) {
            return Err(unsupported(format!(
                "needs {name} {major}.{minor}; update {app} or the plugin"
            )));
        }
        if major != have_major || (major == 0 && minor != have_minor) {
            return Err(unsupported(format!(
                "was built for {name} {major}.{minor}, and this version of {app} runs {name} {have_major}.{have_minor}; update the plugin"
            )));
        }
    }
    Ok(())
}

pub(crate) fn invalid(field: impl Into<String>, reason: impl Into<String>) -> HostError {
    HostError::InvalidManifest {
        field: field.into(),
        reason: reason.into(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Read,
    #[default]
    Edit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSchema {
    pub name: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGrant {
    pub name: String,
    pub folder: PathBuf,
    pub read: bool,
    pub write: bool,
}

pub trait PluginHost {
    fn command_catalog(&self) -> Vec<CommandSchema>;
    fn run_command(&mut self, name: &str, args: &Value) -> Result<Value>;
    fn begin_transaction(&mut self) -> Result<()>;
    fn commit_transaction(&mut self) -> Result<()>;
    fn rollback_transaction(&mut self) -> Result<()>;
    fn granted_folders(&self) -> Vec<FileGrant>;
    fn granted_command_groups(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }
    fn authorize_file_path(&self, path: &Path, write: bool) -> Result<()> {
        let grants = self.granted_folders();
        let relative = grants
            .iter()
            .filter(|grant| if write { grant.write } else { grant.read })
            .filter_map(|grant| path.strip_prefix(&grant.folder).ok())
            .min_by_key(|relative| relative.components().count())
            .unwrap_or(path);
        if relative.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('.'))
        }) {
            return Err(HostError::PermissionDenied {
                permission: format!("hidden file: {}", path.display()),
            });
        }
        Ok(())
    }
    fn log(&mut self, message: &str);
    fn progress(&mut self, fraction: f32, message: &str);
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostLimits {
    pub max_fuel: u64,
    pub max_memory_mib: usize,
}
impl Default for HostLimits {
    fn default() -> Self {
        Self {
            max_fuel: 100_000_000_000,
            max_memory_mib: 128,
        }
    }
}
impl HostLimits {
    pub fn validate(&self) -> Result<()> {
        if self.max_fuel == 0 || !(1..=MAX_MEMORY_MIB).contains(&self.max_memory_mib) {
            return Err(invalid(
                "host_limits",
                "requires positive max_fuel and max_memory_mib 1..4096 (Wasm32)",
            ));
        }
        Ok(())
    }
    pub fn check(&self, requested: &Limits) -> Result<()> {
        self.validate()?;
        requested.validate()?;
        if requested.fuel > self.max_fuel {
            return Err(HostError::LimitExceeded {
                limit: "fuel exceeds user host limits".into(),
            });
        }
        if requested.memory_mib > self.max_memory_mib {
            return Err(HostError::LimitExceeded {
                limit: "memory exceeds user host limits".into(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub fuel: u64,
    pub memory_mib: usize,
    pub deadline_ms: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 10_000_000,
            memory_mib: 64,
            deadline_ms: 5000,
        }
    }
}
impl Limits {
    pub fn validate(&self) -> Result<()> {
        if self.fuel == 0 {
            return Err(invalid("runtime.limits.fuel", "must be positive"));
        }
        if !(1..=MAX_MEMORY_MIB).contains(&self.memory_mib) {
            return Err(invalid(
                "runtime.limits.memory_mib",
                "must be 1..4096 (Wasm32)",
            ));
        }
        if !(1..=30_000).contains(&self.deadline_ms) {
            return Err(invalid(
                "runtime.limits.deadline_ms",
                "must be 1..30000",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Permissions {
    pub files: Vec<FilePermission>,
    pub clock: bool,
    pub command_groups: BTreeSet<String>,
    pub net: bool,
}
impl Permissions {
    pub fn validate(&self) -> Result<()> {
        if self.net {
            return Err(HostError::PermissionDenied {
                permission: "net".into(),
            });
        }
        if self.command_groups.len() > 16
            || self.command_groups.iter().any(|group| !valid_id(group))
        {
            return Err(invalid(
                "runtime.permissions.command_groups",
                "at most 16 lowercase group names",
            ));
        }
        if self.files.len() > 32 {
            return Err(invalid(
                "runtime.permissions.files",
                "at most 32 folder grants",
            ));
        }
        let mut names = BTreeSet::new();
        for grant in &self.files {
            if !valid_id(&grant.name)
                || (!grant.read && !grant.write)
                || !names.insert(&grant.name)
            {
                return Err(invalid(
                    "runtime.permissions.files",
                    "distinct named grants must request read or write",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilePermission {
    pub name: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum UiSlot {
    Menu { name: String },
    Ribbon { name: String },
}

fn empty_input_schema() -> Value {
    serde_json::json!({"type": "object", "properties": {}})
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "EntryDeclaration")]
pub struct Entry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub effect: Effect,
    pub input_schema: Value,
    pub slots: Vec<UiSlot>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EntryDeclaration {
    Legacy(String),
    Declared(DeclaredEntry),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredEntry {
    name: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    effect: Effect,
    #[serde(default = "empty_input_schema")]
    input_schema: Value,
    #[serde(default)]
    slots: Vec<UiSlot>,
}

impl From<EntryDeclaration> for Entry {
    fn from(value: EntryDeclaration) -> Self {
        match value {
            EntryDeclaration::Legacy(name) => Self {
                name,
                label: None,
                description: None,
                effect: Effect::Edit,
                input_schema: empty_input_schema(),
                slots: Vec::new(),
            },
            EntryDeclaration::Declared(value) => Self {
                name: value.name,
                label: value.label,
                description: value.description,
                effect: value.effect,
                input_schema: value.input_schema,
                slots: value.slots,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiManifest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifest {
    pub module: String,
    pub entries: Vec<Entry>,
    pub commands: BTreeSet<String>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub permissions: Permissions,
}

impl RuntimeManifest {
    pub fn validate(&self) -> Result<()> {
        if !valid_id(self.module.strip_suffix(".wasm").unwrap_or("")) {
            return Err(invalid(
                "runtime.module",
                "must be one lowercase .wasm file name",
            ));
        }
        if self.entries.is_empty() || self.entries.len() > 32 {
            return Err(invalid("runtime.entries", "requires 1..32 distinct ids"));
        }
        let mut names = BTreeSet::new();
        for entry in &self.entries {
            if !valid_id(&entry.name) || !names.insert(&entry.name) {
                return Err(invalid("runtime.entries", "requires 1..32 distinct ids"));
            }
            if let Some(label) = &entry.label
                && !plain(label, 64)
            {
                return Err(invalid(
                    "runtime.entries.label",
                    "requires 1..64 printable characters",
                ));
            }
            let field = format!("runtime.entries.{}.input_schema", entry.name);
            if !entry.input_schema.is_object()
                || entry.input_schema.get("type").and_then(Value::as_str) != Some("object")
            {
                return Err(invalid(&field, "entry inputs must be an object schema"));
            }
            validate_list_inputs(&entry.input_schema, &field)?;
            if entry.slots.len() > 16
                || entry.slots.iter().any(|slot| match slot {
                    UiSlot::Menu { name } | UiSlot::Ribbon { name } => !plain(name, 80),
                })
            {
                return Err(invalid(
                    "runtime.entries.slots",
                    "requires at most 16 named menu or ribbon slots",
                ));
            }
        }
        if self.commands.len() > 128 || self.commands.iter().any(|id| !valid_command_name(id)) {
            return Err(invalid(
                "runtime.commands",
                "requires at most 128 exact command names",
            ));
        }
        self.permissions.validate()?;
        self.limits.validate()
    }
}

fn validate_list_inputs(schema: &Value, field: &str) -> Result<()> {
    if let Some(properties) = schema.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| invalid(format!("{field}.properties"), "must be an object"))?;
        for (name, property) in properties {
            let field = format!("{field}.properties.{name}");
            if matches!(
                name.as_str(),
                "selection" | "target" | "targets" | "bodies" | "body_ids"
            ) && property.get("type").and_then(Value::as_str) != Some("array")
            {
                return Err(invalid(&field, "selection and target inputs must be lists"));
            }
            validate_list_inputs(property, &field)?;
        }
    }
    Ok(())
}

pub fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && bytes.first() != Some(&b'-')
        && bytes.last() != Some(&b'-')
}

fn valid_command_name(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
}

pub(crate) fn plain(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value.chars().any(char::is_control)
}

pub fn parse_manifest(text: &[u8], folder: &str) -> Result<Manifest> {
    parse_manifest_for(text, folder, "the app")
}

pub fn parse_manifest_for(text: &[u8], folder: &str, app: &str) -> Result<Manifest> {
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(HostError::LimitExceeded {
            limit: "manifest size exceeds 16 KiB".into(),
        });
    }
    let mut manifest: Manifest =
        serde_json::from_slice(text).map_err(|error| invalid("manifest", error.to_string()))?;
    manifest.name = manifest.name.trim().to_string();
    manifest.version = manifest.version.trim().to_string();
    manifest.description = manifest.description.trim().to_string();
    if !valid_id(&manifest.id) {
        return Err(invalid("id", "must match its lowercase package folder"));
    }
    if manifest.id != folder {
        return Err(invalid(
            "id",
            format!("package id `{}` does not match the folder `{folder}`", manifest.id),
        ));
    }
    if !plain(&manifest.name, 80) {
        return Err(invalid("name", "requires 1..80 printable characters"));
    }
    if !plain(&manifest.version, 32)
        || !manifest
            .version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
    {
        return Err(invalid(
            "version",
            "requires 1..32 alphanumeric, dot, dash or plus characters",
        ));
    }
    if manifest.description.chars().count() > 240
        || manifest.description.chars().any(char::is_control)
    {
        return Err(invalid(
            "description",
            "requires at most 240 printable characters",
        ));
    }
    check_capabilities(&manifest.capabilities, app)?;
    if let Some(runtime) = &manifest.runtime {
        if !manifest
            .capabilities
            .iter()
            .any(|capability| capability.starts_with("commands@"))
        {
            return Err(invalid(
                "capabilities",
                "a plugin with a runtime needs the commands capability: \"capabilities\": [\"commands@0.1\"]",
            ));
        }
        runtime.validate()?;
    }
    if let Some(ui) = &manifest.ui {
        if !manifest
            .capabilities
            .iter()
            .any(|capability| capability == UI_CAPABILITY)
        {
            return Err(invalid(
                "capabilities",
                format!("a plugin declaring a `ui` section needs the `{UI_CAPABILITY}` capability"),
            ));
        }
        let runtime = manifest.runtime.as_ref().ok_or_else(|| {
            invalid(
                "ui",
                "a plugin declaring a `ui` section must also declare a `runtime` with entries",
            )
        })?;
        ui.validate(runtime)?;
    }
    Ok(manifest)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;