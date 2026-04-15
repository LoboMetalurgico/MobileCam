use std::{env, ffi::OsStr, fs::File, io::{BufWriter, Write, stderr}, path::{Component, Path}, process::Stdio};

use phf_codegen::Map;
use walkdir::WalkDir;

fn main() {
    let frontend_dir = Path::new("frontend");
    let frontend_dist_dir = frontend_dir.join("build");

    let npm_command = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };

    run_command(npm_command, &["install"], frontend_dir);
    run_command(npm_command, &["run", "build"], frontend_dir);

    println!("cargo:rerun-if-changed={}", frontend_dir.display());

    let static_frontend_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR not set")).join("frontend.rs");
    let mut static_frontend_file = BufWriter::new(File::create(&static_frontend_path).expect("Failed to create output file"));

    let mut frontend_map = Map::new();

    for entry in WalkDir::new(&frontend_dist_dir).into_iter().filter_map(|f| f.ok()) {
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

        let key = normalize_path(&frontend_dist_dir, path);
        let value = format!("include_bytes!(r#\"{}\"#)", path.canonicalize().expect("Cannot canonicalize static file path").display());

        frontend_map.entry(key, value);
    }

    write!(static_frontend_file, "static FRONTEND: phf::Map<&'static str, &'static [u8]> = {};\n", frontend_map.build()).expect("Failed to write output file");
}

fn run_command<C: AsRef<OsStr>,  AI: IntoIterator<Item = AS>, AS: AsRef<OsStr>, D: AsRef<Path>>(command: C, args: AI, dir: D) {
    let status = std::process::Command::new(command.as_ref())
        .args(args)
        .current_dir(dir)
        .stdout(stderr())
        .stderr(stderr())
        .stdin(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("Failed to run {}: {e}", command.as_ref().display()));

    if !status.success() {
        panic!("{} failed with status: {status}", command.as_ref().display());
    }
}

fn normalize_path<P1: AsRef<Path>, P2: AsRef<Path>>(base: P1, path: P2) -> String {
    let base = base.as_ref();
    let path = path.as_ref();
    let mut result = String::new();

    eprintln!("Normalizing path: {} against base: {}", path.display(), base.display());
    let relative_path = path.strip_prefix(base).unwrap_or(path);

    for component in relative_path.components() {
        match component {
            Component::Normal(os_str) => {
                let s = os_str.to_str().expect("Path component is not valid UTF-8");

                if !s.is_empty() {
                    if !result.is_empty() {
                        result.push('/');
                    }

                    result.push_str(s);
                }
            }
            _ => {
                panic!("Unexpected path component: {component:?}");
            }
        }
    }

    result
}