import zarr
import numpy as np
import os
import shutil

store_path = "climate_data.zarr"
if os.path.exists(store_path):
    shutil.rmtree(store_path)

root = zarr.group(store=store_path)

# Create lat, lon, time
lat = root.create_dataset('lat', shape=(180,), chunks=(180,), dtype='f8')
lat[:] = np.linspace(-90, 90, 180)

lon = root.create_dataset('lon', shape=(360,), chunks=(360,), dtype='f8')
lon[:] = np.linspace(-180, 180, 360)

time = root.create_dataset('time', shape=(365,), chunks=(365,), dtype='i8')
time[:] = np.arange(365) # 365 days

# Create data (time, lat, lon)
z = root.create_dataset(
    'air_temperature',
    shape=(365, 180, 360),
    chunks=(30, 30, 30),
    dtype='f4',
    fill_value=-9999.0
)

# Fill with random data to look real
data = np.random.rand(365, 180, 360).astype('f4') * 30.0 + 273.15
z[:] = data

# Add metadata so zarrduck can recognize it
root.attrs['_ARRAY_DIMENSIONS'] = ['time', 'lat', 'lon']
z.attrs['_ARRAY_DIMENSIONS'] = ['time', 'lat', 'lon']
lat.attrs['_ARRAY_DIMENSIONS'] = ['lat']
lon.attrs['_ARRAY_DIMENSIONS'] = ['lon']
time.attrs['_ARRAY_DIMENSIONS'] = ['time']

print("Generated demo data")