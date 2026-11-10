fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generator = schemars::generate::SchemaSettings::draft07().into_generator();
    let schema = generator.into_root_schema_for::<guic_packager::Config>();
    serde_json::to_writer_pretty(std::io::stdout().lock(), &schema)?;
    Ok(())
}
