import re

with open('extension/src/table_function.rs', 'r') as f:
    content = f.read()

# Remove resolve_dimension_names function
content = re.sub(r'fn resolve_dimension_names\(metadata: &ArrayMetadata, rank: usize\) -> Vec<String> \{.*?\n\}\n\n', '', content, flags=re.DOTALL)

# Remove the test mod entirely since it was just testing resolve_dimension_names
content = re.sub(r'#\[cfg\(test\)\]\nmod tests \{.*?\}\n', '', content, flags=re.DOTALL)

# Remove unused import Arc
content = content.replace('use std::sync::{Arc, Mutex};', 'use std::sync::Mutex;')

with open('extension/src/table_function.rs', 'w') as f:
    f.write(content)
