import zarr
import numpy as np
import os
import shutil

store_path = '/data/test.zarr'
if os.path.exists(store_path):
    shutil.rmtree(store_path)

# Create a 2D array of floats with a fill value
z = zarr.create(
    shape=(1000, 1000),
    chunks=(100, 100),
    dtype='f8',
    store=store_path,
    fill_value=-9999.0
)

# Fill with some data, leaving some as fill_value
data = np.ones((1000, 1000), dtype='f8') * 42.0
# Add some fill values
data[0:100, 0:100] = -9999.0
z[:] = data

# Create coordinate arrays
lat = zarr.create(shape=(1000,), chunks=(100,), dtype='f8', store=store_path + '/lat')
lat[:] = np.linspace(0, 99.9, 1000)

lon = zarr.create(shape=(1000,), chunks=(100,), dtype='f8', store=store_path + '/lon')
lon[:] = np.linspace(0, 99.9, 1000)

print(f"Generated Zarr array at {store_path}")
