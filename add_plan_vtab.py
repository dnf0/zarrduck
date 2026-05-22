import re

with open("extension/src/table_function.rs", "r") as f:
    content = f.read()

vtab_start = content.find("pub struct ReadZarrVTab;")
if vtab_start == -1:
    print("Could not find ReadZarrVTab")
    exit(1)

test_start = content.find("mod tests {", vtab_start)
if test_start == -1:
    print("Could not find test block")
    exit(1)

vtab_code = content[vtab_start:test_start]

# Create the PlanReadZarrVTab code by replacing names and modifying bind/func
plan_code = vtab_code.replace("ReadZarrVTab", "PlanReadZarrVTab")
plan_code = plan_code.replace("ReadZarrInitData", "PlanReadZarrInitData")
plan_code = plan_code.replace("ReadZarrBindData", "PlanReadZarrBindData")

import re

# Replace add_result_column calls
# We want to delete all bind.add_result_column lines and replace them with ours.
plan_code = re.sub(r'bind\.add_result_column.*?;', '', plan_code)
# Re-insert our columns right before Ok(PlanReadZarrBindData
plan_code = plan_code.replace("Ok(PlanReadZarrBindData {", """
        bind.add_result_column("total_chunks", LogicalTypeId::UBigint.into());
        bind.add_result_column("total_bytes", LogicalTypeId::UBigint.into());
        Ok(PlanReadZarrBindData {""")

# We need to add total_chunks and total_bytes to PlanReadZarrBindData.
bind_data_struct = """
pub struct PlanReadZarrBindData {
    pub total_chunks: u64,
    pub total_bytes: u64,
}

pub struct PlanReadZarrInitData {
    pub done: std::sync::atomic::AtomicBool,
}
"""

# Modify the `init` function of PlanReadZarrVTab
init_func_start = plan_code.find("fn init(")
init_func_end = plan_code.find("fn func(")

new_init = """    fn init(_init: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(PlanReadZarrInitData { done: std::sync::atomic::AtomicBool::new(false) })
    }

"""
plan_code = plan_code[:init_func_start] + new_init + plan_code[init_func_end:]

# Modify the `func` function
func_start = plan_code.find("fn func(")
new_func = """    fn func(func: &duckdb::vtab::VTabFunctionData<Self::BindData, Self::InitData>, output: &mut duckdb::core::DataChunkHandle) -> Result<(), Box<dyn std::error::Error>> {
        let init_data = unsafe { &*func.get_init_data::<PlanReadZarrInitData>() };
        if init_data.done.load(std::sync::atomic::Ordering::Relaxed) {
            output.set_len(0);
            return Ok(());
        }

        let bind_data = func.get_bind_data();

        output.flat_vector(0).insert(0, bind_data.total_chunks);
        output.flat_vector(1).insert(0, bind_data.total_bytes);
        output.set_len(1);

        init_data.done.store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }
}
"""
plan_code = plan_code[:func_start] + new_func

# Now we need to modify the `bind` function to calculate total_chunks and total_bytes
# We can just intercept the values right before Ok(PlanReadZarrBindData {

calc_logic = """
        let mut chunk_bounds_min = vec![0; rank];
        let mut chunk_bounds_max = vec![0; rank];
        let mut total_chunks = 1u64;
        for i in 0..rank {
            chunk_bounds_min[i] = bounds_min[i] / chunk_shape[i];
            chunk_bounds_max[i] = bounds_max[i] / chunk_shape[i];
            total_chunks *= (chunk_bounds_max[i] - chunk_bounds_min[i] + 1);
        }
        
        let total_bytes = total_chunks * chunk_volume * bytes_per_element;
"""

# Inject this logic right before the Ok
plan_code = plan_code.replace("""        bind.add_result_column("total_chunks", LogicalTypeId::UBigint.into());""", calc_logic + """        bind.add_result_column("total_chunks", LogicalTypeId::UBigint.into());""")

# Let's replace the whole Ok block.
ok_block_pattern = r'Ok\(PlanReadZarrBindData \{.*?\n    \}\)'
plan_code = re.sub(ok_block_pattern, """Ok(PlanReadZarrBindData {
            total_chunks,
            total_bytes,
        })""", plan_code, flags=re.DOTALL)

# Let's write the whole file back
final_code = content.replace("mod tests {", bind_data_struct + "\n" + plan_code + "\nmod tests {")

with open("extension/src/table_function.rs", "w") as f:
    f.write(final_code)

print("Injected PlanReadZarrVTab")