import duckdb
import os
con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
con.execute(f"LOAD '{os.getcwd()}/target/debug/libgeozarr.dylib'")
res = con.execute("SELECT total_chunks, total_bytes FROM plan_read_zarr('climate_data.zarr/air_temperature', lon_min=-125.0, lat_min=30.0, lon_max=-115.0, lat_max=45.0)").fetchall()
print(res)
