use super::*;

#[test]
fn parses_manifest_with_ribbon_tabs_and_panels() {
    let raw = br#"{
        "id": "gear-toolkit",
        "name": "Gear Toolkit",
        "version": "0.1.0",
        "description": "Parametric spur and helical gear generator",
        "capabilities": ["commands@0.1", "ui@0.1"],
        "runtime": {
            "module": "gear-toolkit.wasm",
            "entries": [
                {
                    "name": "spur-gear",
                    "label": "Spur Gear",
                    "effect": "edit",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "teeth": { "type": "integer" },
                            "module_mm": { "type": "number" }
                        }
                    }
                }
            ],
            "commands": ["solid.extrude", "sketch.create"]
        },
        "ui": {
            "ribbon_tabs": [
                {
                    "id": "gears",
                    "label": "GEARS",
                    "groups": [
                        {
                            "id": "create",
                            "label": "CREATE",
                            "items": [
                                {
                                    "entry": "spur-gear",
                                    "label": "Spur Gear",
                                    "icon": "gear",
                                    "promoted": true
                                }
                            ]
                        }
                    ]
                }
            ],
            "panels": [
                {
                    "id": "gear-inspector",
                    "title": "Gear Calculator",
                    "allowed_zones": ["right"],
                    "default_visible": false
                }
            ]
        }
    }"#;

    let manifest = parse_manifest(raw, "gear-toolkit").expect("valid manifest");
    assert_eq!(manifest.id, "gear-toolkit");
    let ui = manifest.ui.expect("ui manifest present");
    assert_eq!(ui.ribbon_tabs.len(), 1);
    assert_eq!(ui.ribbon_tabs[0].label, "GEARS");
    assert_eq!(ui.ribbon_tabs[0].groups[0].items[0].entry, "spur-gear");
}

#[test]
fn rejects_ui_without_ui_capability() {
    let raw = br#"{
        "id": "bad-plugin",
        "name": "Bad Plugin",
        "version": "0.1.0",
        "capabilities": ["commands@0.1"],
        "runtime": {
            "module": "bad.wasm",
            "entries": ["run"],
            "commands": []
        },
        "ui": {
            "ribbon_tabs": [
                {
                    "id": "tab",
                    "label": "TAB",
                    "groups": [
                        { "id": "grp", "label": "GRP", "items": [{ "entry": "run" }] }
                    ]
                }
            ]
        }
    }"#;

    let err = parse_manifest(raw, "bad-plugin").unwrap_err();
    assert!(matches!(err, HostError::InvalidManifest { ref field, .. } if field == "capabilities"));
}

#[test]
fn rejects_ribbon_item_pointing_to_undeclared_entry() {
    let raw = br#"{
        "id": "bad-entry-ref",
        "name": "Bad Entry Ref",
        "version": "0.1.0",
        "capabilities": ["commands@0.1", "ui@0.1"],
        "runtime": {
            "module": "plugin.wasm",
            "entries": ["valid-entry"],
            "commands": []
        },
        "ui": {
            "ribbon_tabs": [
                {
                    "id": "tab",
                    "label": "TAB",
                    "groups": [
                        { "id": "grp", "label": "GRP", "items": [{ "entry": "missing-entry" }] }
                    ]
                }
            ]
        }
    }"#;

    let err = parse_manifest(raw, "bad-entry-ref").unwrap_err();
    assert!(matches!(
        err,
        HostError::InvalidManifest { ref field, .. } if field == "ui.ribbon_tabs.groups.items.entry"
    ));
}