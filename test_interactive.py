import pexpect
import os
import sys
import time

env = os.environ.copy()
env['ZARRDUCK_LOCAL_STAC'] = 'http://localhost:8080 - Local Demo Catalog'

child = pexpect.spawn('cargo run --bin zarrduck -- search --bbox -122.27,37.77,-122.22,37.81', env=env)
child.logfile = sys.stdout.buffer

try:
    child.expect('Select a STAC Provider:')
    time.sleep(1)
    child.sendline('demo')
    time.sleep(1)
    child.expect('Select a STAC Collection to search:')
    time.sleep(1)
    child.sendline('mock')
    time.sleep(1)
    child.expect('Select a dataset to use:')
    time.sleep(1)
    child.sendline('air_temp')
    time.sleep(1)
    child.expect(pexpect.EOF)
except Exception as e:
    print(e)
