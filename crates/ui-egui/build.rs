//! Optional app-owned catalogs and the existing CI-pinned CJK font faces.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const LOCALES: &[&str] = &["ar", "de", "es", "fr", "ja", "ko", "pl", "pt-br", "zh-hans", "zh-hant"];

fn catalogs(dir: Option<&Path>, required: bool) -> Result<(String, Vec<PathBuf>), String> {
    let mut source = String::from("static CRAFT_I18N: &craft_i18n::Embedded = &[\n");
    let mut paths = Vec::new();
    let Some(dir) = dir else {
        if required {
            return Err("CRAFT_I18N_REQUIRED=1 requires CRAFT_I18N_DIR".into());
        }
        source.push_str("];\n");
        return Ok((source, paths));
    };
    if !dir.is_absolute() {
        return Err("CRAFT_I18N_DIR must be an absolute path".into());
    }
    let mut text = Vec::new();
    for code in LOCALES {
        let common = dir.join("locales").join(code).join("common.tsv");
        let app = dir.join("locales").join(code).join("materialcraft.tsv");
        if !app.is_file() && !required {
            continue;
        }
        let mut include = |p: &Path| -> Result<(String, &'static str), String> {
            use std::io::Read as _;
            let name = p.to_str().ok_or("non-UTF-8 catalog path")?;
            if name.contains(['\n', '\r']) {
                return Err("a catalog path contains a line break".into());
            }
            println!("cargo::rerun-if-changed={name}");
            let file = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err(format!("{} is not a regular catalog file", p.display()));
            }
            let mut bytes = Vec::new();
            file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(format!("{} exceeds the catalog build budget of 16 MiB", p.display()));
            }
            let data = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            paths.push(p.to_path_buf());
            Ok((format!("include_str!({name:?})"), Box::leak(data.into_boxed_str())))
        };
        let (common_include, common_text) = include(&common)?;
        let (app_include, app_text) = include(&app)?;
        text.push((*code, common_text, app_text));
        let _ = writeln!(source, "    ({code:?}, {common_include}, {app_include}),");
    }
    let (lookup, mut problems) = craft_i18n::I18n::new(Box::leak(text.into_boxed_slice()), LOCALES);
    problems.extend(lookup.check_all());
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    source.push_str("];\n");
    Ok((source, paths))
}

fn main() {
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_DIR");
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_REQUIRED");
    let dir = std::env::var_os("CRAFT_I18N_DIR").filter(|s| !s.is_empty()).map(PathBuf::from);
    let required = std::env::var("CRAFT_I18N_REQUIRED").as_deref() == Ok("1");
    let (source, _) = match catalogs(dir.as_deref(), required) {
        Ok(value) => value,
        Err(error) => {
            if required {
                println!("cargo::error={error}");
            } else {
                println!("cargo::warning=building without MaterialCraft catalogs: {error}");
            }
            ("static CRAFT_I18N: &craft_i18n::Embedded = &[];\n".into(), Vec::new())
        }
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("craft_i18n.rs");
    if let Err(error) = std::fs::write(&out, source) {
        println!("cargo::error=writing {}: {error}", out.display());
    }
    // This app's pinned legacy manifest has no UI-role faces. Preserve those verified
    // whole regular CJK faces instead of asking for cuts the input does not supply.
    let embedded = craft_fonts_build::Embed::new()
        .scripts(&["Jpan", "Hans"])
        .filter(|f| f.style == "Regular" && matches!(f.family.as_str(), "BIZ UDPGothic" | "Noto Sans CJK SC"))
        .required(required)
        .write("craft_fonts.rs");
    if let Err(error) = embedded {
        println!("cargo::error={error}");
    }
}
