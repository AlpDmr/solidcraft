# SolidCraft WASM Plugin Development Guide

SolidCraft provides a sandboxed WebAssembly (`wasmi`) plugin architecture that allows third-party extensions to register Ribbon actions, parametric input dialogs, and 3D modeling commands without modifying the core engine.

## 1. Directory Layout & Installation Path

Each plugin resides in a dedicated folder named after its unique `id` inside the local plugins directory:
- **Windows:** `%LOCALAPPDATA%\SolidCraft\plugins\<plugin-id>\`
- **Required Files:**
  - `plugin.json` — Strict manifest declaring metadata, capabilities, runtime entries, and UI placement.
  - `<module>.wasm` — Compiled WebAssembly binary (`wasm32-unknown-unknown`).

## 2. Flexible UI Placement (`ui.placements` & `ui.ribbon_tabs`)

A single plugin can simultaneously place actions in three ways inside `plugin.json`:
1. **Inside a Built-in Tab & Existing Group (`ui.placements`):** Target an existing tab (`MODEL`, `SKETCH`, `SURFACE`, `ASSEMBLE`, `INSERT`, `INSPECT`, `MANAGE`, `VIEW`) and an existing group (`CREATE`, `MODIFY`, `CONSTRUCT`, `COMPONENTS`, etc.).
2. **Inside a Built-in Tab as a Custom Subgroup (`ui.placements`):** Target a built-in tab with a custom `group` and `group_label` to add a dedicated subgroup alongside built-in panels.
3. **Dedicated Top-Level Ribbon Tab (`ui.ribbon_tabs`):** Create one or more standalone top-level Ribbon tabs with multiple groups and buttons.

```json
{
  "id": "gear-toolkit",
  "name": "Gear Toolkit",
  "version": "0.1.0",
  "description": "Parametric spur gear generator",
  "capabilities": ["commands@0.1", "ui@0.1"],
  "runtime": {
    "module": "gear-toolkit.wasm",
    "commands": ["sketch.create", "sketch.line", "sketch.finish", "solid.extrude"],
    "entries": [
      {
        "name": "spur-gear",
        "label": "Spur Gear",
        "input_schema": {
          "type": "object",
          "properties": {
            "teeth": { "type": "integer", "default": 24, "minimum": 6, "maximum": 120 },
            "module_mm": { "type": "number", "default": 2.5, "minimum": 0.2 },
            "helical": { "type": "boolean", "default": false }
          }
        }
      }
    ]
  },
  "ui": {
    "placements": [
      {
        "tab": "MODEL",
        "group": "CREATE",
        "group_label": "Gear Create",
        "items": [
          { "entry": "spur-gear", "label": "Spur Gear", "promoted": true }
        ]
      }
    ],
    "ribbon_tabs": [
      {
        "id": "gears",
        "label": "Gears",
        "groups": [
          {
            "id": "generators",
            "label": "Generators",
            "items": [
              { "entry": "spur-gear", "label": "Spur Gear", "promoted": true }
            ]
          }
        ]
      }
    ]
  }
}
```

## 3. Host ABI Contract (`memory`, `alloc`, `sc_run`)

The `wasmi` executor interacts with the following exported WASM symbols:
- `memory`: Linear WebAssembly memory.
- `alloc(len: i32) -> i32`: Allocates input buffer space for the host to write the entry name and JSON arguments.
- `sc_run(entry_ptr: i32, entry_len: i32, args_ptr: i32, args_len: i32) -> i64`: Executes the requested entry and returns `(ptr << 32) | len` pointing to a UTF-8 JSON payload `{ "commands": [...], "result": {...} }`.

## 4. Security & Sandboxing

- Plugins have zero access to the filesystem, network, or OS system calls (no WASI).
- Commands not listed in `runtime.commands` are rejected at runtime by the engine capability firewall.
- Plugin output payloads are capped at **1 MiB**.

## 5. Building the Example Plugin

```powershell
rustup target add wasm32-unknown-unknown
cd examples/gear-toolkit-wasm
cargo build --target wasm32-unknown-unknown --release
```
