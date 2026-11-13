//! Declarative Ribbon tabs, groups, and panel definitions for third-party plugins.
//! Strictly app-agnostic: contains no egui or SolidCraft dependencies.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{invalid, plain, valid_id, Result, RuntimeManifest};

pub const UI_CAPABILITY: &str = "ui@0.1";
pub const MAX_RIBBON_TABS_PER_PLUGIN: usize = 4;
pub const MAX_GROUPS_PER_TAB: usize = 8;
pub const MAX_BUTTONS_PER_GROUP: usize = 16;
pub const MAX_PANELS_PER_PLUGIN: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiManifest {
    pub ribbon_tabs: Vec<RibbonTabManifest>,
    pub panels: Vec<PluginPanelManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RibbonTabManifest {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub groups: Vec<RibbonGroupManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RibbonGroupManifest {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub items: Vec<RibbonItemManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RibbonItemManifest {
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub promoted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DockZone {
    Left,
    Right,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPanelManifest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub allowed_zones: Vec<DockZone>,
    #[serde(default)]
    pub default_visible: bool,
}

impl UiManifest {
    pub fn validate(&self, runtime: &RuntimeManifest) -> Result<()> {
        if self.ribbon_tabs.len() > MAX_RIBBON_TABS_PER_PLUGIN {
            return Err(invalid("ui.ribbon_tabs", "at most 4 ribbon tabs per plugin"));
        }
        if self.panels.len() > MAX_PANELS_PER_PLUGIN {
            return Err(invalid("ui.panels", "at most 8 panels per plugin"));
        }

        let declared_entries: BTreeSet<&str> =
            runtime.entries.iter().map(|entry| entry.name.as_str()).collect();

        let mut tab_ids = BTreeSet::new();
        for tab in &self.ribbon_tabs {
            if !valid_id(&tab.id) || !tab_ids.insert(tab.id.as_str()) {
                return Err(invalid(
                    "ui.ribbon_tabs.id",
                    "requires distinct lowercase kebab-case tab ids",
                ));
            }
            if !plain(&tab.label, 40) {
                return Err(invalid(
                    "ui.ribbon_tabs.label",
                    "requires 1..40 printable characters",
                ));
            }
            if tab.groups.is_empty() || tab.groups.len() > MAX_GROUPS_PER_TAB {
                return Err(invalid(
                    "ui.ribbon_tabs.groups",
                    "requires 1..8 groups per ribbon tab",
                ));
            }

            let mut group_ids = BTreeSet::new();
            for group in &tab.groups {
                if !valid_id(&group.id) || !group_ids.insert(group.id.as_str()) {
                    return Err(invalid(
                        "ui.ribbon_tabs.groups.id",
                        "requires distinct lowercase group ids",
                    ));
                }
                if !plain(&group.label, 40) {
                    return Err(invalid(
                        "ui.ribbon_tabs.groups.label",
                        "requires 1..40 printable characters",
                    ));
                }
                if group.items.is_empty() || group.items.len() > MAX_BUTTONS_PER_GROUP {
                    return Err(invalid(
                        "ui.ribbon_tabs.groups.items",
                        "requires 1..16 items per ribbon group",
                    ));
                }

                for item in &group.items {
                    if !declared_entries.contains(item.entry.as_str()) {
                        return Err(invalid(
                            "ui.ribbon_tabs.groups.items.entry",
                            format!("references undeclared entry `{}`", item.entry),
                        ));
                    }
                    if let Some(label) = &item.label
                        && !plain(label, 48)
                    {
                        return Err(invalid(
                            "ui.ribbon_tabs.groups.items.label",
                            "requires 1..48 printable characters",
                        ));
                    }
                    if let Some(icon) = &item.icon
                        && !valid_id(icon)
                    {
                        return Err(invalid(
                            "ui.ribbon_tabs.groups.items.icon",
                            "icon id must be lowercase kebab-case",
                        ));
                    }
                }
            }
        }

        let mut panel_ids = BTreeSet::new();
        for panel in &self.panels {
            if !valid_id(&panel.id) || !panel_ids.insert(panel.id.as_str()) {
                return Err(invalid(
                    "ui.panels.id",
                    "requires distinct lowercase panel ids",
                ));
            }
            if !plain(&panel.title, 64) {
                return Err(invalid(
                    "ui.panels.title",
                    "requires 1..64 printable characters",
                ));
            }
        }

        Ok(())
    }
}