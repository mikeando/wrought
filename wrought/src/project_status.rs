use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::{binary16::ContentHash, event_log::EventLog, PackageDirectory, PackageStatus};

pub struct FileRepresentationFromEvents {
    hash: ContentHash,
    dependencies_and_hashes: BTreeMap<PathBuf, Option<ContentHash>>,
}

pub struct ProjectRepresentationFromEvents {
    entries: BTreeMap<PathBuf, FileRepresentationFromEvents>,
}

pub struct ProjectRepresentationFromFilesystem {
    entries: BTreeMap<PathBuf, ContentHash>,
}

#[derive(Debug)]
pub enum FileStatus {
    Untracked,
    Deleted,
    Present { is_changed: bool, is_stale: bool },
}

#[derive(Debug)]
pub struct FileStatusEntry {
    pub path: PathBuf,
    pub status: FileStatus,
}

#[derive(Debug)]
pub struct ProjectStatus {
    pub file_statuses: Vec<FileStatusEntry>,
    pub package_statuses: Vec<PackageStatus>,
}

pub fn get_all_file_hashes_in_directory<P: Into<PathBuf>>(
    fs: &dyn xfs::Xfs,
    path: P,
) -> anyhow::Result<BTreeMap<PathBuf, ContentHash>> {
    let mut result = BTreeMap::new();
    // I hate recursion - this should use a stack instead. But for now it's nice and easy.
    fs.on_each_entry(&path.into(), &mut |fs, e| {
        let md = e.metadata()?;
        if md.is_dir() {
            let mut child_hashes = get_all_file_hashes_in_directory(fs, e.path())?;
            result.append(&mut child_hashes);
        } else if md.is_file() {
            let mut reader = fs.reader(&e.path())?;
            let mut content = vec![];
            reader.read_to_end(&mut content)?;
            result.insert(e.path(), ContentHash::from_content(&content));
        }
        Ok(())
    })?;
    Ok(result)
}

pub fn build_rep_from_fs<P: Into<PathBuf>>(
    fs: &dyn xfs::Xfs,
    project_root: P,
) -> anyhow::Result<ProjectRepresentationFromFilesystem> {
    let project_root = project_root.into();
    let file_hashes = get_all_file_hashes_in_directory(fs, &project_root)?;
    // Remove the project_root prefix from them all.
    let file_hashes = file_hashes
        .into_iter()
        .map(|(k, v)| (k.strip_prefix(&project_root).unwrap().to_path_buf(), v))
        .collect();
    Ok(ProjectRepresentationFromFilesystem {
        entries: file_hashes,
    })
}

pub fn build_rep_from_event_log(
    event_log: &dyn EventLog,
) -> anyhow::Result<ProjectRepresentationFromEvents> {
    let mut all_event_groups = event_log.all_event_groups()?;
    all_event_groups.sort_by_key(|g| g.id);

    let mut result = ProjectRepresentationFromEvents {
        entries: BTreeMap::new(),
    };

    for group in all_event_groups {
        for event in group.events {
            // TODO: For now we only track dependencies on files - not metadata.
            let mut dependencies = BTreeMap::new();
            match event.event_type {
                crate::events::EventType::WriteFile(write_file_event) => {
                    match write_file_event.after_hash {
                        Some(hash) => {
                            result.entries.insert(
                                write_file_event.path,
                                FileRepresentationFromEvents {
                                    hash,
                                    dependencies_and_hashes: dependencies.clone(),
                                },
                            );
                        }
                        None => {
                            // Represents removal of the file
                            result.entries.remove(&write_file_event.path);
                        }
                    }
                }
                crate::events::EventType::ReadFile(read_file_event) => {
                    dependencies.insert(read_file_event.path, read_file_event.hash);
                }
                crate::events::EventType::GetMetadata(_) => {}
                crate::events::EventType::SetMetadata(_) => {}
            }
        }
    }
    Ok(result)
}

pub fn get_project_status(
    event_log: &dyn EventLog,
    fs: &dyn xfs::Xfs,
    project_root: &Path,
) -> anyhow::Result<ProjectStatus> {
    let mut file_statuses = vec![];
    let rep1 = build_rep_from_event_log(event_log)?;
    let rep2 = build_rep_from_fs(fs, project_root)?;

    let mut all_paths: BTreeSet<&PathBuf> = rep1.entries.keys().collect();
    for p in rep2.entries.keys() {
        all_paths.insert(p);
    }

    for p in all_paths {
        let e1 = rep1.entries.get(p);
        let e2 = rep2.entries.get(p);
        let status = match (e1, e2) {
            (None, None) => unreachable!(),
            (Some(_), None) => {
                // We have an entry in the event log, but no local copy.
                // Lets just mark that as deleted
                FileStatus::Deleted
            }
            (None, Some(_)) => {
                // We have a local copy, but it has no entry in the event log.
                FileStatus::Untracked
            }
            (Some(e1), Some(e2)) => {
                // We have both a local copy and a tracked version.
                // We have to check if it has changes, and if its inputs have changed.
                let is_changed = e1.hash != *e2;
                let mut is_stale = false;
                for (dep_path, dep_hash) in &e1.dependencies_and_hashes {
                    if rep2.entries.get(dep_path) != dep_hash.as_ref() {
                        is_stale = true;
                        break;
                    }
                }
                FileStatus::Present {
                    is_changed,
                    is_stale,
                }
            }
        };
        file_statuses.push(FileStatusEntry {
            path: p.clone(),
            status,
        });
    }

    // MODIFY THIS....
    let package_dir = PackageDirectory {
        path: project_root.join(".wrought").join("packages"),
    };

    let mut package_statuses = vec![];
    let packages = package_dir.packages(fs);
    for package in packages {
        let package = package?;
        package_statuses.push(package.status(fs));
    }

    Ok(ProjectStatus {
        file_statuses,
        package_statuses,
    })
}
