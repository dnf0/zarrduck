import os

def replace_in_file(filepath):
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        return

    new_content = content.replace('zarrduck', 'zarrduck')
    new_content = new_content.replace('zarrduck', 'zarrduck')
    new_content = new_content.replace('zarrduck', 'zarrduck')

    if new_content != content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(new_content)
        print(f"Updated {filepath}")

for root, dirs, files in os.walk('.'):
    # skip .git, target, __pycache__, graphify-out
    if '.git' in root or 'target' in root or '__pycache__' in root or 'graphify-out' in root or '.worktrees' in root:
        continue
    for file in files:
        if file.endswith('.dylib') or file.endswith('.so') or file.endswith('.zip') or file.endswith('.png') or file.endswith('.gif'):
            continue
        filepath = os.path.join(root, file)
        replace_in_file(filepath)
