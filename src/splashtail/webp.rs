use std::io::Write;

/// Module to convert images to webp format
/// 
/// This internally just calls cwebp/gif2webp, so you need to have that installed
pub fn image_to_webp(name: &str, b: &[u8]) -> Result<Vec<u8>, crate::Error> {
    let mut cmd = {
        if name.ends_with("gif") {
            let mut c = std::process::Command::new("gif2webp");
            c.arg("-q").arg("100").arg("-o").arg("-").arg("-");

            c
        } else {
            let mut c = std::process::Command::new("cwebp");
            c.arg("-q").arg("100").arg("-o").arg("-").arg("-");

            c
        }
    };

    let mut child = cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(b)?;

    let output = child.wait_with_output()?;

    Ok(output.stdout)
}