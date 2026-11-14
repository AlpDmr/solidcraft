//! Dynamic Ribbon UI rendering and schema-driven parameter dialog for third-party plugins.

use std::collections::BTreeMap;
use egui::{Align2, FontId, Id, Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Map, Value, json};
use solidcraft_engine::PluginRibbonItem;
use solidcraft_theme::Tokens;
use crate::{SolveApp, icons};

pub const PLUGIN_TAB_PREFIX: &str = "plugin:";

pub fn is_plugin_tab(tab_id: &str) -> bool {
    tab_id.starts_with(PLUGIN_TAB_PREFIX)
}

pub fn encode_plugin_tab(key: &str) -> String {
    format!("{PLUGIN_TAB_PREFIX}{key}")
}

pub fn decode_plugin_tab(tab_id: &str) -> Option<&str> {
    tab_id.strip_prefix(PLUGIN_TAB_PREFIX)
}

#[derive(Clone, Debug)]
struct PendingPluginCall {
    command_id: String,
    label: String,
    fields: Vec<SchemaField>,
}

#[derive(Clone, Debug)]
struct SchemaField {
    name: String,
    kind: FieldKind,
    value: String,
    bool_val: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Number,
    Integer,
    Boolean,
    Text,
}

fn extract_fields(schema: &Value) -> Vec<SchemaField> {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    props.iter().map(|(name, spec)| {
        let ty = spec.get("type").and_then(Value::as_str).unwrap_or("string");
        let default = spec.get("default");
        let (kind, value, bool_val) = match ty {
            "number" => (FieldKind::Number, default.and_then(Value::as_f64).map(|v| v.to_string()).unwrap_or_else(|| "1.0".into()), false),
            "integer" => (FieldKind::Integer, default.and_then(Value::as_i64).map(|v| v.to_string()).unwrap_or_else(|| "1".into()), false),
            "boolean" => (FieldKind::Boolean, String::new(), default.and_then(Value::as_bool).unwrap_or(false)),
            _ => (FieldKind::Text, default.and_then(Value::as_str).unwrap_or("").to_string(), false),
        };
        SchemaField { name: name.clone(), kind, value, bool_val }
    }).collect()
}

pub fn trigger_item(app: &mut SolveApp, ctx: &egui::Context, item: &PluginRibbonItem) {
    let fields = extract_fields(&item.input_schema);
    if fields.is_empty() {
        let _ = app.run(&item.command_id, json!({}));
    } else {
        let modal = PendingPluginCall { command_id: item.command_id.clone(), label: item.label.clone(), fields };
        ctx.data_mut(|d| d.insert_temp(Id::unique("sc_plugin_modal"), modal));
    }
}
pub fn paint_plugin_tab_body(app: &mut SolveApp, ui: &mut egui::Ui, area: Rect, tab_key: &str) {
    let t = Tokens::get();
    let tabs = app.session.plugin_ribbon_tabs();
    let Some(tab) = tabs.into_iter().find(|tb| tb.key == tab_key) else {
        ui.painter().text(area.center(), Align2::CENTER_CENTER, "Plugin tab is no longer active.", FontId::proportional(t.font_body), t.text_dim);
        return;
    };
    let mut x = area.left() + 8.0;
    let group_top = area.top() + 4.0;
    let group_h = (area.height() - 8.0).max(40.0);

    for group in &tab.groups {
        let item_w = 68.0_f32;
        let pad = 8.0_f32;
        let count = group.items.len().max(1) as f32;
        let group_w = (count * item_w + pad * 2.0).max(84.0);
        let grect = Rect::from_min_size(pos2(x, group_top), vec2(group_w, group_h));
        ui.painter().rect_stroke(grect, 4.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);

        let mut bx = grect.left() + pad;
        for item in &group.items {
            let brect = Rect::from_min_size(pos2(bx, grect.top() + 4.0), vec2(item_w - 4.0, group_h - 22.0));
            let resp = ui.interact(brect, Id::unique(("sc_plugin_btn", &item.command_id)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(brect, 4.0, t.nav_hover);
            }
            let icon_rect = Rect::from_center_size(pos2(brect.center().x, brect.top() + 18.0), vec2(20.0, 20.0));
            let icon_name = item.icon.as_deref().unwrap_or("plugin");
            icons::paint_on(ui.painter(), icon_rect, icon_name, Some(true), 1.0);
            ui.painter().text(
                pos2(brect.center().x, brect.bottom() - 8.0),
                Align2::CENTER_CENTER,
                &item.label,
                FontId::proportional(t.font_label),
                t.text,
            );
            if resp.clicked() {
                trigger_item(app, ui.ctx(), item);
            }
            bx += item_w;
        }

        ui.painter().text(
            pos2(grect.center().x, grect.bottom() - 8.0),
            Align2::CENTER_CENTER,
            &group.label,
            FontId::proportional((t.font_label - 1.0).max(9.0)),
            t.text_dim,
        );
        x += group_w + 6.0;
    }
}

pub fn show_dialog(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(mut modal) = ctx.data(|d| d.get_temp::<PendingPluginCall>(Id::unique("sc_plugin_modal"))) else {
        return;
    };
    let mut open = true;
    let mut execute = false;
    let mut cancel = false;

    egui::Window::new(&modal.label)
        .id(Id::unique("sc_plugin_param_window"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(&modal.command_id).small().weak());
            ui.add_space(6.0);
            egui::Grid::new("sc_plugin_param_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                for field in &mut modal.fields {
                    ui.label(&field.name);
                    match field.kind {
                        FieldKind::Boolean => { ui.checkbox(&mut field.bool_val, ""); }
                        _ => { ui.text_edit_singleline(&mut field.value); }
                    }
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Run").clicked() { execute = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });

    if execute {
        let mut obj = Map::new();
        for f in &modal.fields {
            let val = match f.kind {
                FieldKind::Number => Value::from(f.value.trim().parse::<f64>().unwrap_or(0.0)),
                FieldKind::Integer => Value::from(f.value.trim().parse::<i64>().unwrap_or(0)),
                FieldKind::Boolean => Value::Bool(f.bool_val),
                FieldKind::Text => Value::String(f.value.clone()),
            };
            obj.insert(f.name.clone(), val);
        }
        let _ = app.run(&modal.command_id, Value::Object(obj));
        ctx.data_mut(|d| d.remove::<PendingPluginCall>(Id::unique("sc_plugin_modal")));
    } else if cancel || !open {
        ctx.data_mut(|d| d.remove::<PendingPluginCall>(Id::unique("sc_plugin_modal")));
    } else {
        ctx.data_mut(|d| d.insert_temp(Id::unique("sc_plugin_modal"), modal));
    }
}

/// Scans `{data}/plugins/<id>/plugin.json` on native targets and registers valid plugins into `app.session`.
pub fn sync_installed_plugins(app: &mut SolveApp) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = crate::plugins::plugins_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let Some(folder) = path.file_name().and_then(|n| n.to_str()) else { continue; };
                let manifest_path = path.join("plugin.json");
                if let Ok(bytes) = std::fs::read(&manifest_path) {
                    let executor = craft_plugin::parse_manifest(&bytes, folder)
                        .ok()
                        .and_then(|m| m.runtime)
                        .and_then(|rt| {
                            let wasm_bytes = std::fs::read(path.join(&rt.module)).ok()?;
                            solidcraft_engine::plugins::wasm_executor(wasm_bytes, rt.commands.into_iter().collect()).ok()
                        });
                    let _ = app.session.register_plugin_json(&bytes, folder, executor);
                }
            }
        }
    }
    let disabled: BTreeMap<String, bool> = app.preferences.disabled_plugins.iter().map(|id| (id.clone(), false)).collect();
    for (id, enabled) in disabled {
        let _ = app.session.set_plugin_enabled(&id, enabled);
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod plugin_ribbon_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encodes_decodes_plugin_tabs_and_extracts_schema_fields() {
        let key = encode_plugin_tab("gear-toolkit:gears");
        assert!(is_plugin_tab(&key));
        assert_eq!(decode_plugin_tab(&key), Some("gear-toolkit:gears"));
        assert!(!is_plugin_tab("MODEL"));

        let schema = json!({
            "type": "object",
            "properties": {
                "teeth": { "type": "integer", "default": 24 },
                "module_mm": { "type": "number", "default": 2.5 },
                "helical": { "type": "boolean", "default": true }
            }
        });
        let fields = extract_fields(&schema);
        assert_eq!(fields.len(), 3);
    }
}