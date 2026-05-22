import pexpect
import os
import sys
import time

env = os.environ.copy()

def run():
    print("Testing plot wizard...")
    child = pexpect.spawn('cargo run --bin zarrduck -- plot analysis.duckdb', env=env)
    child.logfile = sys.stdout.buffer
    child.expect('Which table would you like to plot?')
    time.sleep(0.5)
    child.sendline('extracted_data')
    child.expect('Select variables to analyze')
    time.sleep(0.5)
    child.send('\x1b[B') # down to lat
    time.sleep(0.2)
    child.send('\x1b[B') # down to lon
    time.sleep(0.2)
    child.send('\x1b[B') # down to value
    time.sleep(0.2)
    child.send(' ') # select value
    time.sleep(0.5)
    child.sendline('') # Enter to confirm
    child.expect('Choose plot type:')
    time.sleep(0.5)
    child.sendline('Heatmap')
    child.expect(pexpect.EOF)
    print("\nAll done!")

try:
    run()
except Exception as e:
    print(f"Error: {e}")
