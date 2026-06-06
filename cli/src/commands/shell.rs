use color_eyre::eyre::Result as EyreResult;
use std::process::Command;

pub fn run_shell(db_path: String) -> EyreResult<()> {
    let ext_name = "eider.duckdb_extension";
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

    let ext_path = match std::env::var("EIDER_EXTENSION_PATH") {
        Ok(p) if !p.is_empty() => p,
        _ => candidate_paths
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| cwd.join("target").join("debug").join(ext_name))
            .to_string_lossy()
            .into_owned(),
    };

    // Probe whether this DuckDB CLI can actually load the extension. The
    // extension is version-locked to the DuckDB it was built against, so a CLI
    // of a different version (or no extension built) can't load it. Rather than
    // let a failed LOAD abort the whole shell, only include it when the probe
    // succeeds and otherwise launch with just spatial.
    let escaped_ext = ext_path.replace('\'', "''");
    let load_ext = Command::new("duckdb")
        .arg("-unsigned")
        .arg("-c")
        .arg(format!("LOAD '{}';", escaped_ext))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let init_commands = if load_ext {
        format!("INSTALL spatial; LOAD spatial; LOAD '{}';", escaped_ext)
    } else {
        println!(
            "Note: the eider GeoZarr extension could not be loaded into your local \
             DuckDB CLI (version mismatch or extension not built); continuing without it."
        );
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
