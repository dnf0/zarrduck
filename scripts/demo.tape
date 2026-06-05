Output docs/static/img/demo-v2.gif

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
Type "export PATH=$PWD/target/debug:$PATH"
Enter
Type "export PS1='❯ '"
Enter
Type "rm -f *.duckdb"
Enter
Type "pkill -f mock_stac.py || true"
Enter
Type "python3 scripts/mock_stac.py 8123 &"
Enter
Sleep 1s
Type "export EIDER_LOCAL_STAC='http://localhost:8123 - Local Demo Catalog'"
Enter
Type "python3 scripts/generate_demo_data.py && cargo build -p eider && cargo duckdb-ext build"
Enter
Sleep 15s
Type "source scripts/header.sh"
Enter
Type "clear"
Enter
Show

# Step 1: STAC Discovery
Hide
Type "header '1. Discover cloud-native GeoZarr assets via STAC'"
Enter
Show
Sleep 1s
Type "eider search --bbox -122.27,37.77,-122.22,37.81"
Enter
Sleep 2s
Type "demo"
Sleep 1s
Enter
Sleep 2s
Type "cmip6"
Sleep 1s
Enter
Sleep 4s

# Step 2: Zonal Extraction
Hide
Type "header '2. Extract and join spatial data'"
Enter
Show
Sleep 1s
Type "eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson --out analysis.duckdb --yes"
Enter
Sleep 8s

# Step 3: Temporal Resampling
Hide
Type "header '3. Automatically aggregate time-series data using interactive menus'"
Enter
Show
Sleep 1s
Type "eider resample analysis.duckdb monthly.duckdb"
Enter
Sleep 2s
Type "mon"
Sleep 1s
Enter
Sleep 2s
Type "avg"
Sleep 1s
Enter
Sleep 3s

# Step 4: Interactive Analysis
Hide
Type "header '4. Drop into a spatial-enabled shell'"
Enter
Show
Sleep 1s
Type "eider shell monthly.duckdb"
Enter
Sleep 3s
Type "SELECT time, lat, lon, value FROM resampled_data LIMIT 5;"
Enter
Sleep 4s

Type ".exit"
Enter
Sleep 2s

# Step 5: Data Visualization
Hide
Type "header '5. Interactive data visualization'"
Enter
Show
Sleep 1s
Type "eider plot analysis.duckdb"
Enter
Sleep 2s
Type "extracted"
Sleep 1s
Enter
Sleep 2s
Down 3
Sleep 500ms
Space
Sleep 500ms
Enter
Sleep 2s
Type "Heatmap"
Sleep 1s
Enter
Sleep 8s
