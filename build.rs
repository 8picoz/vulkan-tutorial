use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), anyhow::Error> {
    SpirvBuilder::new("./shaders/rust-shader/", "spirv-unknown-vulkan1.2")
        .print_metadata(MetadataPrintout::Full)
        .build()?;

    Ok(())
}
