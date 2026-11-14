//! Declarative Ribbon tabs, built-in tab placements, groups, and panel definitions for third-party plugins.
//! Strictly app-agnostic: contains no egui or SolidCraft dependencies.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{invalid, plain, valid_id, Result, RuntimeManifest};

pub const UI_CAPABILITY: &str = "ui@0.1";
pub const MAX_RIBBON_TABS_PER_PLUGIN: usize = 4;
pub const MAX_GROUPS_PER_TAB: usize = 8;
pub const MAX_BUTTONS_PER_GROUP: usize = 16;
pub const MAX_PANELS_PER_PLUGIN: usize = 8;
pub const MAX_PLACEMENTS_PER_PLUGIN: usize = 16;

/// Top-level UI declaration inside `plugin.json`.
/// Plugins may declare standalone `ribbon_tabs`, inject buttons/groups into built-in tabs via
/// `placements`, and/or register dockable `panels` simultaneously.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiManifest {
    pub ribbon_tabs: Vec<RibbonTabManifest>,
    pub placements: Vec<UiPlacementManifest>,
    pub panels: Vec<PluginPanelManifest>,
}

/// Targets a built-in SolidCraft Ribbon tab (e.g., `MODEL`, `SKETCH`, `SURFACE`, `ASSEMBLE`)
/// and either appends items into an existing group (e.g., `CREATE`, `MODIFY`) or creates a
/// dedicated subgroup within that tab.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiPlacementManifest {
    /// Target built-in tab identifier (case-insensitive, e.g. `"MODEL"`, `"SKETCH"`, `"SURFACE"`, `"ASSEMBLE"`).
    pub tab: String,
    /// Target group/panel identifier within the tab (e.g. `"CREATE"`, `"MODIFY"`, or a custom subgroup id like `"gears"`).
    pub group: String,
    /// Optional human-readable heading when creating a custom subgroup inside a built-in tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_label: Option<String>,
    /// One or more plugin entry buttons to place in the target group.
    pub items: Vec<RibbonItemManifest>,
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
        if self.placements.len() > MAX_PLACEMENTS_PER_PLUGIN {
            return Err(invalid("ui.placements", "at most 16 built-in tab placements per plugin"));
        }
        if self.panels.len() > MAX_PANELS_PER_PLUGIN {
            return Err(invalid("ui.panels", "at most 8 panels per plugin"));
        }

        let declared_entries: BTreeSet<&str> =
            runtime.entries.iter().map(|entry| entry.name.as_str()).collect();

        for placement in &self.placements {
            if !plain(&placement.tab, 40) {
                return Err(invalid("ui.placements.tab", "requires 1..40 printable characters"));
            }
            if !plain(&placement.group, 40) {
                return Err(invalid("ui.placements.group", "requires 1..40 printable characters"));
            }
            if let Some(gl) = &placement.group_label
                && !plain(gl, 40)
            {
                return Err(invalid("ui.placements.group_label", "requires 1..40 printable characters"));
            }
            if placement.items.is_empty() || placement.items.len() > MAX_BUTTONS_PER_GROUP {
                return Err(invalid("ui.placements.items", "requires 1..16 items per placement"));
            }
            for item in &placement.items {
                validate_ribbon_item(item, &declared_entries, "ui.placements.items")?;
            }
        }

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
                    validate_ribbon_item(item, &declared_entries, "ui.ribbon_tabs.groups.items")?;
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

fn validate_ribbon_item(
    item: &RibbonItemManifest,
    declared_entries: &BTreeSet<&str>,
    prefix: &str,
) -> Result<()> {
    if !declared_entries.contains(item.entry.as_str()) {
        return Err(invalid(
            format!("{prefix}.entry"),
            format!("references undeclared entry `{}`", item.entry),
        ));
    }
    if let Some(label) = &item.label
        && !plain(label, 48)
    {
        return Err(invalid(
            format!("{prefix}.label"),
            "requires 1..48 printable characters",
        ));
    }
    if let Some(icon) = &item.icon
        && !valid_id(icon)
    {
        return Err(invalid(
            format!("{prefix}.icon"),
            "icon id must be lowercase kebab-case",
        ));
    }
    Ok(())
}