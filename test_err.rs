fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res: Result<(), String> = Err("some error".to_string());
    res?;
    Ok(())
}
