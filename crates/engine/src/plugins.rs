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

/// Projected UI placement targeting a built-in SolidCraft Ribbon tab (e.g. `MODEL`, `SKETCH`, `SURFACE`, `ASSEMBLE`)
/// and either an existing group (`CREATE`, `MODIFY`) or a custom plugin subgroup within that tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPlacement {
    pub plugin_id: String,
    pub plugin_name: String,
    pub tab: String,
    pub group: String,
    pub group_label: String,
    pub items: Vec<PluginRibbonItem>,
}

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
    /// Returns all active plugin placements targeting built-in Ribbon tabs and groups.
    pub fn placements(&self) -> Vec<PluginPlacement> {
        self.plugins.values().filter(|p| p.enabled).filter_map(|p| Some((&p.manifest, &p.manifest.runtime.as_ref()?.entries, p.manifest.ui.as_ref()?)))
            .flat_map(|(m, entries, ui)| {
                ui.placements.iter().map(move |pl| PluginPlacement {
                    plugin_id: m.id.clone(),
                    plugin_name: m.name.clone(),
                    tab: pl.tab.to_ascii_uppercase(),
                    group: pl.group.to_ascii_uppercase(),
                    group_label: pl.group_label.clone().unwrap_or_else(|| pl.group.clone()),
                    items: pl.items.iter().filter_map(|i| project_ribbon_item(m, entries, i)).collect(),
                })
            })
            .collect()
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
    /// Returns standalone custom Ribbon tabs registered by enabled plugins.
    pub fn plugin_ribbon_tabs(&self) -> Vec<PluginRibbonTab> { self.plugins.ribbon_tabs() }
    /// Returns plugin button groups injected into built-in Ribbon tabs (`MODEL`, `SKETCH`, `SURFACE`, `ASSEMBLE`, etc.).
    pub fn plugin_placements(&self) -> Vec<PluginPlacement> { self.plugins.placements() }
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


#[derive(Deserialize)]
struct WasmPluginOutput {
    #[serde(default)]
    commands: Vec<WasmCommandCall>,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct WasmCommandCall {
    id: String,
    #[serde(default)]
    args: Value,
}

#[cfg(not(target_arch = "wasm32"))]
/// Creates a sandboxed WebAssembly (`wasmi`) [`PluginExecutor`] for a plugin module.
///
/// # Security & Sandboxing
/// - Executes pure compute modules without WASI, filesystem, or network imports.
/// - Enforces a strict command allowlist (`allowed_commands`): any command emitted by the plugin
///   that is not declared in `plugin.json` (`runtime.commands`) is rejected with [`EngineError::Disabled`].
/// - Caps returned JSON output buffers at 1 MiB to prevent unbounded memory allocation.
pub fn wasm_executor(wasm_bytes: Vec<u8>, allowed_commands: Vec<String>) -> Result<PluginExecutor> {
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, &wasm_bytes)
        .map_err(|e| EngineError::Other(format!("invalid plugin wasm module: {e}")))?;
    let allowed: BTreeSet<String> = allowed_commands.into_iter().collect();

    Ok(Arc::new(move |session: &mut Session, entry: &str, params: &Value| {
        let mut store = wasmi::Store::new(&engine, ());
        let instance = wasmi::Linker::<()>::new(&engine)
            .instantiate(&mut store, &module)
            .map_err(|e| EngineError::Other(format!("wasm instantiate failed: {e}")))?
            .start(&mut store)
            .map_err(|e| EngineError::Other(format!("wasm start failed: {e}")))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| EngineError::Other("plugin wasm must export `memory`".into()))?;

        let args_bytes = serde_json::to_vec(params)
            .map_err(|e| EngineError::Internal(entry.into(), e.to_string()))?;
        let entry_bytes = entry.as_bytes();

        let (entry_ptr, args_ptr) = if let Ok(alloc) = instance.get_typed_func::<i32, i32>(&store, "alloc") {
            let ep = alloc
                .call(&mut store, entry_bytes.len() as i32)
                .map_err(|e| EngineError::Other(format!("wasm alloc failed: {e}")))?;
            memory
                .write(&mut store, ep as usize, entry_bytes)
                .map_err(|e| EngineError::Other(format!("wasm memory write failed: {e}")))?;
            let ap = alloc
                .call(&mut store, args_bytes.len() as i32)
                .map_err(|e| EngineError::Other(format!("wasm alloc failed: {e}")))?;
            memory
                .write(&mut store, ap as usize, &args_bytes)
                .map_err(|e| EngineError::Other(format!("wasm memory write failed: {e}")))?;
            (ep, ap)
        } else {
            (0, 0)
        };

        let sc_run = instance
            .get_typed_func::<(i32, i32, i32, i32), i64>(&store, "sc_run")
            .map_err(|e| EngineError::Other(format!("plugin wasm must export `sc_run(i32, i32, i32, i32) -> i64`: {e}")))?;

        let packed = sc_run
            .call(&mut store, (entry_ptr, entry_bytes.len() as i32, args_ptr, args_bytes.len() as i32))
            .map_err(|e| EngineError::Other(format!("wasm execution trapped: {e}")))?;

        let out_ptr = ((packed as u64) >> 32) as usize;
        let out_len = (packed as u64 & 0xFFFF_FFFF) as usize;
        if out_len > 1024 * 1024 {
            return Err(EngineError::Other("plugin output exceeds 1 MiB limit".into()));
        }
        let mut out_buf = vec![0u8; out_len];
        memory
            .read(&store, out_ptr, &mut out_buf)
            .map_err(|e| EngineError::Other(format!("wasm output memory read out of bounds: {e}")))?;

        let output: WasmPluginOutput = serde_json::from_slice(&out_buf)
            .map_err(|e| EngineError::Other(format!("plugin returned invalid JSON protocol: {e}")))?;

        if let Some(err_msg) = output.error {
            return Err(EngineError::Other(err_msg));
        }

        for cmd in &output.commands {
            if !allowed.contains(&cmd.id) {
                return Err(EngineError::Disabled(
                    cmd.id.clone(),
                    format!("plugin is not permitted to invoke command `{}` (not in runtime.commands)", cmd.id),
                ));
            }
        }

        let mut last_result = Value::Null;
        for cmd in output.commands {
            let args = if cmd.args.is_null() { serde_json::json!({}) } else { cmd.args };
            last_result = session.execute(&cmd.id, &args)?;
        }
        Ok(if output.result.is_null() { last_result } else { output.result })
    }))
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

    #[test]
    fn wasm_executor_runs_allowed_commands_and_blocks_disallowed() {
        let payload = r#"{"commands":[{"id":"solid.cylinder","args":{"radius":30.0,"height":12.0,"body_name":"SpurGearBlank"}}],"result":{"created":"SpurGearBlank"}}"#;
        let len = payload.len();
        let wat_src = format!(
            r#"(module
                (memory (export "memory") 1)
                (data (i32.const 64) "{escaped}")
                (func (export "sc_run") (param i32 i32 i32 i32) (result i64)
                    (i64.or
                        (i64.shl (i64.const 64) (i64.const 32))
                        (i64.const {len})
                    )
                )
            )"#,
            escaped = payload.replace('"', "\\\""),
            len = len
        );
        let wasm_bytes = wat::parse_str(&wat_src).expect("valid wat");

        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let dir = std::path::PathBuf::from(local_app_data)
                .join("SolidCraft")
                .join("plugins")
                .join("gear-toolkit");
            if dir.exists() {
                let _ = std::fs::write(dir.join("gear-toolkit.wasm"), &wasm_bytes);
            }
        }

        // 1. Disallowed command must be blocked by the capability firewall
        let blocked_exec = wasm_executor(wasm_bytes.clone(), vec!["sketch.create".into()]).expect("executor");
        let mut session = Session::new(solidcraft_doc::Document::new("test"));
        let err = blocked_exec(&mut session, "spur-gear", &serde_json::json!({})).unwrap_err();
        assert!(err.to_string().contains("not permitted to invoke command `solid.cylinder`"), "{err}");

        // 2. Allowed command executes and creates geometry in the session
        let allowed_exec = wasm_executor(wasm_bytes, vec!["solid.cylinder".into()]).expect("executor");
        let out = allowed_exec(&mut session, "spur-gear", &serde_json::json!({"teeth": 24})).expect("allowed run");
        assert_eq!(out["created"], "SpurGearBlank");
    }
}
