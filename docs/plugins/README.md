# SolidCraft WASM Eklenti Geliştirme Kılavuzu

SolidCraft, üçüncü taraf araçların çekirdek motoru değiştirmeden özel Ribbon sekmeleri, parametrik giriş pencereleri ve 3B modelleme komutları eklemesini sağlayan bir WebAssembly (wasmi) eklenti mimarisi sunar.

## 1. Dizin Yapısı ve Kurulum Yolu
- **Windows:** %LOCALAPPDATA%\SolidCraft\plugins\<plugin-id>\
- **Gerekli Dosyalar:** plugin.json ve <module>.wasm

## 2. Manifest (plugin.json) Kuralları
craft-plugin ayrıştırıcısı katı alan doğrulaması (deny_unknown_fields) uygular:
- Üst düzey alanlar: id, name, version, description, capabilities, runtime, ui
- capabilities formatı: name@major.minor (ör. "commands@0.1", "ui@0.1")
- runtime alanları: module, entries, commands, limits, permissions

## 3. Host ABI Sözleşmesi
- memory: Doğrusal WASM belleği
- alloc(len: i32) -> i32: Giriş JSON baytları için bellek ayırır
- sc_run(entry_ptr: i32, entry_len: i32, args_ptr: i32, args_len: i32) -> i64: Çıktı JSON adres ve uzunluğunu (ptr << 32) | len olarak döndürür

## 4. Örnek Projeyi Derleme
cd examples/gear-toolkit-wasm
cargo build --target wasm32-unknown-unknown --release
