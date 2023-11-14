/// Module to convert images to webp format
/// 
/// This internally just calls cwebp/gif2webp, so you need to have that installed
pub fn image_to_webp(name: &str, fpath: String, b: &[u8]) -> Result<String, crate::Error> {
    // Create temp file
    let tmp_id = crate::crypto::gen_random(128);

    let tmp = std::env::temp_dir().join("pconv_".to_owned()+&tmp_id);

    // Write to temp file
    std::fs::write(&tmp, b)?;

    let mut cmd = {
        if name.ends_with("gif") {
            let mut c = std::process::Command::new("gif2webp");
            c.args(
                vec![
                    "-q",
                    "100",
                    "-m",
                    "3",
                    tmp.to_str().ok_or("Invalid temp file path")?,
                    "-o",
                    &fpath,
                    "-v"
                ]
            );

            c
        } else {
            let mut c = std::process::Command::new("cwebp");
            c.args(
                vec![
                    "-q",
                    "100",
                    tmp.to_str().ok_or("Invalid temp file path")?,
                    "-o",
                    &fpath,
                    "-v"
                ]
            );

            c
        }
    };

    // Now run the command
    let output = cmd.output()?;
    

    // Remove temp file
    std::fs::remove_file(&tmp)?;

    if output.status.success() {
        Ok(output.stdout.iter().map(|b| *b as char).collect::<String>() + &output.stderr.iter().map(|b| *b as char).collect::<String>())
    } else {
        Err(format!("Failed to convert image: {} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).into())
    }
}