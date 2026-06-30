use risio::{ImageType, bindings::IMAGE};

fn main() -> anyhow::Result<()> {
    IMAGE::create_new_image_from_scratch(
        "myimname",
        2,
        &[3, 4, 0],
        risio::DataType::F64,
        3,
        ImageType::image(),
        2,
    )?;
    Ok(())
}
