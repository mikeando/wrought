use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use xfs::Xfs;

pub struct PackageDirectory {
    pub path: PathBuf,
}

impl PackageDirectory {
    pub fn packages(&self, fs: &dyn xfs::Xfs) -> Vec<anyhow::Result<Package>> {
        let mut result = vec![];

        let mut f = |_fs: &dyn Xfs, entry: &dyn xfs::XfsDirEntry| -> anyhow::Result<()> {
            let md = match entry.metadata() {
                Err(e) => {
                    result.push(
                        Err(e).with_context(|| format!("getting metadata for {:?}", entry.path())),
                    );
                    return Ok(());
                }
                Ok(md) => md,
            };
            if !md.is_dir() {
                result.push(Err(anyhow!(
                    "package directory entry {:?} is not a directory",
                    entry.path()
                )));
                return Ok(());
            }
            result.push(Ok(Package { path: entry.path() }));
            Ok(())
        };

        if let Err(e) = fs.on_each_entry(&self.path, &mut f) {
            result.push(
                Err(e).with_context(|| format!("while reading packages from {:?}", self.path)),
            );
        }
        result
    }
}


#[derive(Debug)]
pub struct PackageStatus {
    pub package: Package,
    pub entries: Vec<anyhow::Result<PackageStatusEntry>>,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub path: PathBuf,
}

impl Package {
    pub fn status(&self, fs: &dyn xfs::Xfs) -> PackageStatus {
        let status_dir = self.path.join("status");
        let mut entries = vec![];

        let mut f = |fs: &dyn Xfs, entry: &dyn xfs::XfsDirEntry| -> anyhow::Result<()> {
            let md = match entry.metadata() {
                Ok(md) => md,
                Err(e) => {
                    entries.push(
                        Err(e).with_context(|| format!("getting metadata for {:?}", entry.path())),
                    );
                    return Ok(());
                }
            };
            if !md.is_file() {
                entries.push(Err(anyhow!(
                    "status entry {:?} is not a file",
                    entry.path()
                )));
                return Ok(());
            }

            entries.push(PackageStatusEntry::read_from(fs, &entry.path()));
            Ok(())
        };

        if let Err(e) = fs.on_each_entry(&status_dir, &mut f) {
            entries.push(
                Err(e).with_context(|| format!("while reading statuses from {:?}", status_dir)),
            );
        }
        PackageStatus {
            package: self.clone(),
            entries,
        }
    }

    pub fn name(&self) -> String {
        self.path.file_name().unwrap().to_str().unwrap().to_string()
    }
}

#[derive(Debug)]
pub struct PackageStatusEntry {
    pub path: PathBuf,
    pub title: String,
    pub status: String,
    pub next_steps: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageStatusEntryRepr {
    pub title: String,
    pub status: String,
    pub next_steps: Vec<String>,
}

impl PackageStatusEntry {
    pub fn read_from(fs: &dyn xfs::Xfs, p: &Path) -> anyhow::Result<PackageStatusEntry> {
        let mut content = String::new();
        fs.reader(p)?.read_to_string(&mut content)?;
        let content: PackageStatusEntryRepr = toml::from_str(&content)?;

        Ok(PackageStatusEntry {
            path: p.to_path_buf(),
            title: content.title,
            status: content.status,
            next_steps: content.next_steps,
        })
    }

    pub fn name(&self) -> String {
        self.path.file_name().unwrap().to_string_lossy().to_string()
    }
}