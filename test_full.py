import pexpect
import os
import sys
import time

env = os.environ.copy()
env['ZARRDUCK_LOCAL_STAC'] = 'http://localhost:8080 - Local Demo Catalog'

def run():
    print("Testing search...")
    child = pexpect.spawn('cargo run --bin zarrduck -- search --bbox -122.27,37.77,-122.22,37.81', env=env)
    child.logfile = sys.stdout.buffer
    child.expect('Select a STAC Provider:')
    time.sleep(0.5)
    child.sendline('demo')
    child.expect('Select a STAC Collection to search:')
    time.sleep(0.5)
    child.sendline('mock')
    child.expect('Select a dataset to use:')
    time.sleep(0.5)
    child.sendline('air_temp')
    child.expect(pexpect.EOF)

    print("\nTesting extract...")
    child = pexpect.spawn('cargo run --bin zarrduck -- extract climate_data.zarr/air_temperature scripts/demo_region.geojson --out analysis.duckdb', env=env)
    child.logfile = sys.stdout.buffer
    child.expect('Proceed with extraction?')
    time.sleep(0.5)
    child.sendline('')
    child.expect(pexpect.EOF)

    print("\nTesting resample...")
    child = pexpect.spawn('cargo run --bin zarrduck -- resample analysis.duckdb monthly.duckdb', env=env)
    child.logfile = sys.stdout.buffer
    child.expect('Select temporal resampling frequency:')
    time.sleep(0.5)
    child.sendline('mon')
    child.expect('Select aggregation function:')
    time.sleep(0.5)
    child.sendline('avg')
    child.expect(pexpect.EOF)
    print("\nAll done!")

try:
    run()
except Exception as e:
    print(f"Error: {e}")
