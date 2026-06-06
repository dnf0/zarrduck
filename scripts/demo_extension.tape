Output docs/static/img/demo-extension.gif

Set FontSize 16
Set FontFamily "Geist Mono, Berkeley Mono, Fira Code, JetBrains Mono, Menlo, Apple Color Emoji"
Set Width 1200
Set Height 900
Set Padding 18
Set LineHeight 1.12
Set TypingSpeed 28ms
Set CursorBlink false
Set Framerate 30
Set Theme "Dracula"

Hide
Type "export PS1='❯ '"
Enter
Type "python3 scripts/generate_demo_data.py >/dev/null 2>&1; cargo build -p eider >/dev/null 2>&1; cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension >/dev/null 2>&1"
Enter
Sleep 20s
Type "cp target/debug/eider.duckdb_extension /tmp/eider.duckdb_extension"
Enter
Type "source scripts/header.sh"
Enter
Type "clear"
Enter
Show

# Beat 1: Load the extension into a plain DuckDB shell
Hide
Type "header '1. Load the eider extension into DuckDB'"
Enter
Show
Sleep 1s
Type "duckdb -unsigned"
Enter
Sleep 1500ms
Type "LOAD '/tmp/eider.duckdb_extension';"
Enter
Sleep 2s

# Beat 2: Inspect the Zarr array as a SQL table
Hide
Type "header '2. Inspect the array as a SQL table'"
Enter
Show
Sleep 1s
Type "SELECT array_shape, chunk_shape, data_type FROM read_zarr_metadata('climate_data.zarr/air_temperature');"
Enter
Sleep 3s

# Beat 3: Run analytical SQL directly over the Zarr array
Hide
Type "header '3. Run analytical SQL directly over the Zarr array'"
Enter
Show
Sleep 1s
Type "SELECT lat, AVG(value) AS mean_temp FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50) GROUP BY lat ORDER BY lat LIMIT 8;"
Enter
Sleep 5s
Type ".exit"
Enter
Sleep 2s
