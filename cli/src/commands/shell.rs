use color_eyre::eyre::Result as EyreResult;
use std::process::Command;

pub fn run_shell(db_path: String) -> EyreResult<()> {
    let ext_name = "eider_extension.duckdb_extension";
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mut candidate_paths = vec![
        cwd.join("target").join("debug").join(ext_name),
        cwd.parent()
            .unwrap_or(&cwd)
            .join("target")
            .join("debug")
            .join(ext_name),
    ];

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidate_paths.push(parent.join(ext_name));
            if let Some(grandparent) = parent.parent() {
                candidate_paths.push(grandparent.join(ext_name));
            }
        }
    }

    let ext_path = candidate_paths
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| cwd.join("target").join("debug").join(ext_name))
        .to_string_lossy()
        .into_owned();

    let duckdb_version = Command::new("duckdb").arg("-version").output();
    let mut load_ext = false;
    if let Ok(out) = duckdb_version {
        let version_str = String::from_utf8_lossy(&out.stdout);
        if version_str.starts_with("v1.1.") {
            load_ext = true;
        }
    }

    let init_commands = if load_ext {
        format!("INSTALL spatial; LOAD spatial; LOAD '{}';", ext_path)
    } else {
        println!("Note: Local DuckDB CLI version differs from v1.1.x. The GeoZarr extension will not be loaded.");
        "INSTALL spatial; LOAD spatial;".to_string()
    };

    println!("Starting DuckDB shell...");
    let status = Command::new("duckdb")
        .arg(&db_path)
        .arg("-unsigned")
        .arg("-cmd")
        .arg(&init_commands)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("DuckDB shell exited with status: {}", s),
        Err(e) => eprintln!(
            "Failed to launch 'duckdb' CLI. Is it installed in your PATH? Error: {}",
            e
        ),
    }

    Ok(())
}
