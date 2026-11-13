use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use craft_plugin::{parse_manifest, valid_id, DockZone, Effect, Entry, Manifest, PluginPanelManifest, RibbonGroupManifest, RibbonItemManifest, RibbonTabManifest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{EngineError, Result, Session};

pub type PluginExecutor = Arc<dyn Fn(&mut Session, &str, &Value) -> Result<Value> + Send + Sync + 'static>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRibbonTab { pub key: String, pub plugin_id: String, pub plugin_name: String, pub tab_id: String, pub label: String, pub groups: Vec<PluginRibbonGroup> }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRibbonGroup { pub id: String, pub label: String, pub items: Vec<PluginRibbonItem> }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRibbonItem { pub command_id: String, pub entry: String, pub label: String, pub icon: Option<String>, pub promoted: bool, pub effect: Effect, pub input_schema: Value }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPanelInfo { pub key: String, pub plugin_id: String, pub plugin_name: String, pub panel_id: String, pub title: String, pub allowed_zones: Vec<DockZone>, pub default_visible: bool }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginSummary { pub id: String, pub name: String, pub version: String, pub description: String, pub enabled: bool, pub capabilities: Vec<String>, pub entries: Vec<PluginEntrySummary>, pub ribbon_tab_count: usize, pub panel_count: usize }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntrySummary { pub command_id: String, pub name: String, pub label: String, pub description: Option<String>, pub effect: Effect, pub input_schema: Value }

#[derive(Clone)]
pub struct RegisteredPlugin { pub manifest: Manifest, pub enabled: bool, pub executor: Option<PluginExecutor> }

impl RegisteredPlugin {
    pub fn summary(&self) -> PluginSummary {
        let entries = self.manifest.runtime.as_ref().map(|rt| rt.entries.iter().map(|e| PluginEntrySummary {
            command_id: format!("ext.{}.{}", self.manifest.id, e.name), name: e.name.clone(), label: e.label.clone().unwrap_or_else(|| e.name.clone()), description: e.description.clone(), effect: e.effect, input_schema: e.input_schema.clone(),
        }).collect()).unwrap_or_default();
        let (ribbon_tab_count, panel_count) = self.manifest.ui.as_ref().map(|ui| (ui.ribbon_tabs.len(), ui.panels.len())).unwrap_or((0, 0));
        PluginSummary { id: self.manifest.id.clone(), name: self.manifest.name.clone(), version: self.manifest.version.clone(), description: self.manifest.description.clone(), enabled: self.enabled, capabilities: self.manifest.capabilities.clone(), entries, ribbon_tab_count, panel_count }
    }
}

#[derive(Clone, Default)]
pub struct PluginRegistry { plugins: BTreeMap<String, RegisteredPlugin>, disabled: BTreeSet<String> }

impl PluginRegistry {
    pub fn new() -> Self { Self::default() }
    pub fn register(&mut self, manifest: Manifest, executor: Option<PluginExecutor>) -> Result<PluginSummary> {
        let id = manifest.id.clone(); let enabled = !self.disabled.contains(&id);
        let record = RegisteredPlugin { manifest, enabled, executor }; let summary = record.summary();
        self.plugins.insert(id, record); Ok(summary)
    }
    pub fn register_from_json(&mut self, bytes: &[u8], folder: &str, executor: Option<PluginExecutor>) -> Result<PluginSummary> {
        let manifest = parse_manifest(bytes, folder).map_err(|err| EngineError::BadParams { cmd: "plugin.register".into(), msg: err.to_string() })?;
        self.register(manifest, executor)
    }
    pub fn unregister(&mut self, plugin_id: &str) -> bool { self.plugins.remove(plugin_id).is_some() }
    pub fn set_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<()> {
        if enabled { self.disabled.remove(plugin_id); } else { self.disabled.insert(plugin_id.to_string()); }
        let plugin = self.plugins.get_mut(plugin_id).ok_or_else(|| EngineError::Other(format!("unknown plugin `{plugin_id}`")))?;
        plugin.enabled = enabled; Ok(())
    }
    pub fn get(&self, plugin_id: &str) -> Option<&RegisteredPlugin> { self.plugins.get(plugin_id) }
    pub fn list(&self) -> Vec<PluginSummary> { self.plugins.values().map(RegisteredPlugin::summary).collect() }
    pub fn ribbon_tabs(&self) -> Vec<PluginRibbonTab> {
        self.plugins.values().filter(|p| p.enabled).filter_map(|p| Some((&p.manifest, &p.manifest.runtime.as_ref()?.entries, p.manifest.ui.as_ref()?)))
            .flat_map(|(m, entries, ui)| ui.ribbon_tabs.iter().map(move |t| project_ribbon_tab(m, entries, t))).collect()
    }
    pub fn panels(&self) -> Vec<PluginPanelInfo> {
        self.plugins.values().filter(|p| p.enabled).filter_map(|p| Some((&p.manifest, p.manifest.ui.as_ref()?)))
            .flat_map(|(m, ui)| ui.panels.iter().map(move |panel| project_panel(m, panel))).collect()
    }
}
fn project_ribbon_tab(m: &Manifest, entries: &[Entry], t: &RibbonTabManifest) -> PluginRibbonTab {
    PluginRibbonTab { key: format!("{}:{}", m.id, t.id), plugin_id: m.id.clone(), plugin_name: m.name.clone(), tab_id: t.id.clone(), label: t.label.clone(), groups: t.groups.iter().map(|g| project_ribbon_group(m, entries, g)).collect() }
}
fn project_ribbon_group(m: &Manifest, entries: &[Entry], g: &RibbonGroupManifest) -> PluginRibbonGroup {
    PluginRibbonGroup { id: g.id.clone(), label: g.label.clone(), items: g.items.iter().filter_map(|i| project_ribbon_item(m, entries, i)).collect() }
}
fn project_ribbon_item(m: &Manifest, entries: &[Entry], i: &RibbonItemManifest) -> Option<PluginRibbonItem> {
    let e = entries.iter().find(|e| e.name == i.entry)?;
    Some(PluginRibbonItem { command_id: format!("ext.{}.{}", m.id, e.name), entry: e.name.clone(), label: i.label.clone().or_else(|| e.label.clone()).unwrap_or_else(|| e.name.clone()), icon: i.icon.clone(), promoted: i.promoted, effect: e.effect, input_schema: e.input_schema.clone() })
}
fn project_panel(m: &Manifest, p: &PluginPanelManifest) -> PluginPanelInfo {
    PluginPanelInfo { key: format!("{}:{}", m.id, p.id), plugin_id: m.id.clone(), plugin_name: m.name.clone(), panel_id: p.id.clone(), title: p.title.clone(), allowed_zones: p.allowed_zones.clone(), default_visible: p.default_visible }
}
pub fn parse_plugin_command_id(id: &str) -> Option<(&str, &str)> {
    let (plugin_id, entry) = id.strip_prefix("ext.")?.split_once('.')?;
    (valid_id(plugin_id) && valid_id(entry)).then_some((plugin_id, entry))
}

impl Session {
    pub fn register_plugin(&mut self, manifest: Manifest, executor: Option<PluginExecutor>) -> Result<PluginSummary> { self.plugins.register(manifest, executor) }
    pub fn register_plugin_json(&mut self, bytes: &[u8], folder: &str, executor: Option<PluginExecutor>) -> Result<PluginSummary> { self.plugins.register_from_json(bytes, folder, executor) }
    pub fn unregister_plugin(&mut self, plugin_id: &str) -> bool { self.plugins.unregister(plugin_id) }
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<()> { self.plugins.set_enabled(plugin_id, enabled) }
    pub fn plugin_ribbon_tabs(&self) -> Vec<PluginRibbonTab> { self.plugins.ribbon_tabs() }
    pub fn plugin_panels(&self) -> Vec<PluginPanelInfo> { self.plugins.panels() }
    pub fn list_plugins(&self) -> Vec<PluginSummary> { self.plugins.list() }

    pub(crate) fn try_execute_plugin_command(&mut self, id: &str, params: &Value) -> Option<Result<Value>> {
        match id {
            "plugin.list" => Some(serde_json::to_value(self.plugins.list()).map_err(|e| EngineError::Internal(id.into(), e.to_string()))),
            "plugin.toggle" => {
                let Some(pid) = params.get("id").and_then(Value::as_str) else { return Some(Err(EngineError::BadParams { cmd: id.into(), msg: "missing `id`".into() })); };
                let Some(en) = params.get("enabled").and_then(Value::as_bool) else { return Some(Err(EngineError::BadParams { cmd: id.into(), msg: "missing `enabled`".into() })); };
                Some(self.plugins.set_enabled(pid, en).map(|()| serde_json::json!({ "id": pid, "enabled": en })))
            }
            "plugin.unregister" => {
                let Some(pid) = params.get("id").and_then(Value::as_str) else { return Some(Err(EngineError::BadParams { cmd: id.into(), msg: "missing `id`".into() })); };
                Some(Ok(serde_json::json!({ "id": pid, "removed": self.plugins.unregister(pid) })))
            }
            _ => {
                let (pid, entry) = parse_plugin_command_id(id)?;
                Some(self.execute_plugin_entry(id, pid, entry, params))
            }
        }
    }

    fn execute_plugin_entry(&mut self, full_id: &str, pid: &str, entry_name: &str, params: &Value) -> Result<Value> {
        let plugin = self.plugins.get(pid).ok_or_else(|| EngineError::UnknownCommand(full_id.to_string()))?.clone();
        if !plugin.enabled { return Err(EngineError::Disabled(full_id.to_string(), format!("plugin `{pid}` is disabled"))); }
        let rt = plugin.manifest.runtime.as_ref().ok_or_else(|| EngineError::Disabled(full_id.to_string(), format!("plugin `{pid}` has no runtime")))?;
        if !rt.entries.iter().any(|e| e.name == entry_name) { return Err(EngineError::UnknownCommand(full_id.to_string())); }
        let exec = plugin.executor.ok_or_else(|| EngineError::Disabled(full_id.to_string(), format!("plugin `{pid}` has no runtime backend")))?;
        exec(self, entry_name, params)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod plugin_bridge_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registers_plugin_projects_ribbon_tabs_and_executes_command() {
        let mut session = Session::default();
        let manifest = br#"{"id":"weld-pro","name":"Weld Pro","version":"1.0.0","capabilities":["commands@0.1","ui@0.1"],"runtime":{"module":"weld-pro.wasm","entries":[{"name":"fillet-weld","label":"Fillet Weld","input_schema":{"type":"object","properties":{"leg_mm":{"type":"number"}}}}],"commands":["solid.extrude"]},"ui":{"ribbon_tabs":[{"id":"weld","label":"WELD","groups":[{"id":"joint","label":"WELD JOINTS","items":[{"entry":"fillet-weld","icon":"weld","promoted":true}]}]}]}}"#;
        let exec = Arc::new(|_: &mut Session, entry: &str, p: &Value| Ok(json!({ "entry": entry, "leg_mm": p.get("leg_mm").and_then(Value::as_f64).unwrap_or(0.0) })));
        let summary = session.register_plugin_json(manifest, "weld-pro", Some(exec)).unwrap();
        assert_eq!(summary.ribbon_tab_count, 1);
        assert_eq!(session.plugin_ribbon_tabs()[0].label, "WELD");
        let reply = session.execute("ext.weld-pro.fillet-weld", &json!({ "leg_mm": 6.5 })).unwrap();
        assert_eq!(reply["leg_mm"], 6.5);
        session.execute("plugin.toggle", &json!({ "id": "weld-pro", "enabled": false })).unwrap();
        assert!(session.plugin_ribbon_tabs().is_empty());
    }
}