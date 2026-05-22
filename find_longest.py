import os
import glob

def find_longest_functions():
    longest = []
    
    for filepath in glob.glob("**/*.rs", recursive=True):
        if "target/" in filepath:
            continue
            
        with open(filepath, 'r') as f:
            lines = f.readlines()
            
        in_function = False
        func_name = ""
        func_start = 0
        brace_count = 0
        
        for i, line in enumerate(lines):
            stripped = line.strip()
            
            if not in_function and (stripped.startswith("fn ") or stripped.startswith("pub fn ") or stripped.startswith("pub async fn ") or stripped.startswith("async fn ")):
                in_function = True
                func_start = i
                # Extract basic name
                parts = stripped.split('fn ')
                if len(parts) > 1:
                    func_name = parts[1].split('(')[0].split('<')[0].strip()
                else:
                    func_name = "unknown"
                brace_count = line.count('{') - line.count('}')
            elif in_function:
                brace_count += line.count('{') - line.count('}')
                
                if brace_count <= 0: # Function ended
                    length = i - func_start + 1
                    longest.append((length, func_name, filepath, func_start + 1))
                    in_function = False
                    
    longest.sort(reverse=True, key=lambda x: x[0])
    
    print("Top 5 longest functions:")
    for length, name, path, line in longest[:5]:
        print(f"{length} lines: {name} in {path}:{line}")

find_longest_functions()
