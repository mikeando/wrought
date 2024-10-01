use std::path::{Path, PathBuf};
use std::{fs, io};

pub fn copy_dir_all_with_filters<F, D>(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    file_filter: F,
    dir_filter: D,
) -> io::Result<()>
where
    F: Fn(&PathBuf, usize) -> bool,
    D: Fn(&PathBuf, usize) -> bool,
{
    // Apply dir_filter before processing the directory
    if !dir_filter(&src.as_ref().to_path_buf(), 0) {
        return Ok(());
    }

    let mut stack = Vec::new();
    stack.push((src.as_ref().to_path_buf(), dst.as_ref().to_path_buf(), 0)); // Initialize with depth 0

    while let Some((current_src, current_dst, depth)) = stack.pop() {
        fs::create_dir_all(&current_dst)?;

        for entry in fs::read_dir(&current_src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = current_dst.join(entry.file_name());
            let file_type = entry.file_type()?;

            if file_type.is_dir() && dir_filter(&src_path, depth + 1) {
                // Push the directory onto the stack if it passes the filter
                stack.push((src_path, dst_path, depth + 1));
            } else if file_type.is_file() && file_filter(&src_path, depth) {
                // Copy the file only if it passes the file filter
                fs::copy(src_path, dst_path)?;
            }
        }
    }
    Ok(())
}
