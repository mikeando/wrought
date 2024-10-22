use std::path::{Path, PathBuf};

pub fn copy_dir_all_with_filters<F, D>(
    fs: &mut dyn xfs::Xfs,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    file_filter: F,
    dir_filter: D,
) -> anyhow::Result<()>
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
        fs.create_dir_all(&current_dst)?;

        fs.on_each_entry_mut(&current_src, &mut |fs: &mut dyn xfs::Xfs,
                                                 entry: &dyn xfs::XfsDirEntry|
         -> anyhow::Result<()> {
            let src_path = entry.path();
            let dst_path = current_dst.join(src_path.file_name().unwrap());
            let md = entry.metadata()?;

            if md.is_dir() && dir_filter(&src_path, depth + 1) {
                // Push the directory onto the stack if it passes the filter
                stack.push((src_path, dst_path, depth + 1));
            } else if md.is_file() && file_filter(&src_path, depth) {
                // Copy the file only if it passes the file filter
                fs.copy(&src_path, &dst_path)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}
