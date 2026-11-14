use std::alloc::{alloc as std_alloc, Layout};
use std::f64::consts::TAU;
use std::fmt::Write;
use std::slice;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }
    let layout = Layout::from_size_align(len as usize, 1).unwrap();
    unsafe { std_alloc(layout) as i32 }
}

fn parse_number(json: &str, key: &str, default: f64) -> f64 {
    let needle = format!("\"{}\":", key);
    let Some(idx) = json.find(&needle) else { return default; };
    let rest = json[idx + needle.len()..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().unwrap_or(default)
}

fn parse_bool(json: &str, key: &str, default: bool) -> bool {
    let needle = format!("\"{}\":", key);
    let Some(idx) = json.find(&needle) else { return default; };
    let rest = json[idx + needle.len()..].trim_start();
    if rest.starts_with("true") {
        true
    } else if rest.starts_with("false") {
        false
    } else {
        default
    }
}

#[no_mangle]
pub extern "C" fn sc_run(entry_ptr: i32, entry_len: i32, args_ptr: i32, args_len: i32) -> i64 {
    let entry = unsafe {
        let bytes = slice::from_raw_parts(entry_ptr as *const u8, entry_len.max(0) as usize);
        core::str::from_utf8(bytes).unwrap_or("spur-gear")
    };
    let args = unsafe {
        let bytes = slice::from_raw_parts(args_ptr as *const u8, args_len.max(0) as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let response = match entry {
        "spur-gear" => build_spur_gear_commands(args),
        _ => format!(r#"{{"error":"unknown entry `{}`"}}"#, entry),
    };

    let mut out = response.into_bytes().into_boxed_slice();
    let ptr = out.as_mut_ptr() as u64;
    let len = out.len() as u64;
    core::mem::forget(out);
    ((ptr << 32) | (len & 0xFFFF_FFFF)) as i64
}

fn build_spur_gear_commands(args: &str) -> String {
    let teeth = (parse_number(args, "teeth", 24.0).round() as usize).clamp(6, 120);
    let module_mm = parse_number(args, "module_mm", 2.5).clamp(0.2, 50.0);
    let _helical = parse_bool(args, "helical", false);

    let pitch_r = 0.5 * module_mm * (teeth as f64);
    let outer_r = pitch_r + module_mm;
    let root_r = (pitch_r - 1.25 * module_mm).max(module_mm * 0.5);
    let thickness = (module_mm * 4.0).max(2.0);

    // Kapalı dişli profili noktalarını oluştur (her diş için 4 köşe: kök -> tepe -> tepe -> kök)
    let mut pts = Vec::with_capacity(teeth * 4);
    for i in 0..teeth {
        let base = (i as f64) * TAU / (teeth as f64);
        let step = TAU / (teeth as f64);
        let corners = [
            (root_r, base + step * 0.08),
            (outer_r, base + step * 0.24),
            (outer_r, base + step * 0.56),
            (root_r, base + step * 0.72),
        ];
        for (r, ang) in corners {
            pts.push((r * ang.cos(), r * ang.sin()));
        }
    }

    let mut points_json = String::from("[");
    for (idx, (x, y)) in pts.iter().enumerate() {
        if idx > 0 {
            points_json.push(',');
        }
        let _ = write!(points_json, "[{:.4},{:.4}]", x, y);
    }
    points_json.push(']');

    format!(
        r#"{{"commands":[{{"id":"sketch.create","args":{{"plane":"XY"}}}},{{"id":"sketch.line","args":{{"points":{pts},"closed":true}}}},{{"id":"sketch.finish","args":{{}}}},{{"id":"solid.extrude","args":{{"distance":{dist:.4},"operation":"new"}}}}],"result":{{"teeth":{teeth},"module_mm":{m:.4},"pitch_diameter_mm":{pd:.4}}}}}"#,
        pts = points_json,
        dist = thickness,
        teeth = teeth,
        m = module_mm,
        pd = pitch_r * 2.0
    )
}