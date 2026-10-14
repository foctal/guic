fn main() -> Result<(), Box<dyn std::error::Error>> {
    guic_build::build()?;
    Ok(())
}
