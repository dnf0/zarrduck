use rexpect::spawn;

#[test]
fn test_cli_search_interactive() {
    // This requires the eider binary to be built already. 
    // We typically get the binary path from `env!("CARGO_BIN_EXE_eider")`,
    // which is set by cargo when running integration tests.
    let bin_path = env!("CARGO_BIN_EXE_eider");
    
    // Spawn the search command with a 10s timeout to allow network requests to finish
    let mut p = spawn(&format!("{} search", bin_path), Some(10000)).unwrap();
    
    // Wait for the prompt for STAC provider
    p.exp_regex("(?i)Select a STAC Provider").unwrap();
    
    // Send an arrow down to select a different provider (or just enter to select the first one)
    p.send_line("").unwrap();
    
    // Next it will try to fetch the collections and prompt for collection.
    // However, this requires a network request. To avoid flaky tests, we'll
    // just assert it starts fetching or prompts for collection.
    // If we just want to ensure it handles interactivity, reaching the prompt
    // is often enough, but let's see if we can get to the next prompt.
    // Let's just expect it reaches the "Fetching collections" or "Select a collection" prompt.
    // To make it robust against network, we might just expect "Fetching".
    
    let result = p.exp_regex("(?i)Select a STAC Collection|(?i)error");
    assert!(result.is_ok(), "Failed to reach the collection prompt or error gracefully");
}
