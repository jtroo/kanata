//! File watching for configuration hot-reload.
//!
//! Replaces the basic file watcher with comprehensive support for include files
//! and dynamic watcher restart when include files change during reload.

use crate::kanata::Kanata;
use anyhow::Result;
use notify_debouncer_mini::{
    DebounceEventResult, DebouncedEventKind, new_debouncer, notify::RecursiveMode,
};
use parking_lot::Mutex;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Discover include files by parsing config files for (include "path") statements.
/// This is a simple parser that looks for include statements without full parsing.
pub fn discover_include_files(cfg_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut include_files = Vec::new();

    for cfg_path in cfg_paths {
        if let Ok(content) = fs::read_to_string(cfg_path) {
            // Simple regex-like parsing for include statements
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("(include") && trimmed.contains('"') {
                    // Extract the path between quotes
                    if let Some(start) = trimmed.find('"') {
                        if let Some(end) = trimmed[start + 1..].find('"') {
                            let include_path = &trimmed[start + 1..start + 1 + end];
                            let mut path = PathBuf::from(include_path);

                            // If path is relative, make it relative to the config file directory
                            if !path.is_absolute() {
                                if let Some(parent) = cfg_path.parent() {
                                    path = parent.join(path);
                                }
                            }

                            if path.exists() {
                                include_files.push(path);
                                log::debug!(
                                    "Discovered include file: {}",
                                    include_files.last().unwrap().display()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    include_files
}

/// Start comprehensive file watching for configuration files and included files.
/// This replaces the basic file watcher with full include file support and dynamic restart.
pub fn start_file_watcher(kanata_arc: Arc<Mutex<Kanata>>) -> Result<()> {
    // Get paths from kanata
    let cfg_paths = {
        let k = kanata_arc.lock();
        k.cfg_paths.clone()
    };

    // Discover include files and update kanata
    let included_files = discover_include_files(&cfg_paths);
    {
        let mut k = kanata_arc.lock();
        k.included_files = included_files.clone();
    }

    // Create the watcher and store it in the Kanata struct
    let debouncer = create_debouncer(kanata_arc.clone(), &cfg_paths, &included_files)?;

    // Store the debouncer in the Kanata struct
    {
        let mut k = kanata_arc.lock();
        k.file_watcher = Some(debouncer);
    }

    Ok(())
}

/// Create a new file watcher debouncer with the given file lists.
/// This is used both for initial setup and for restarting the watcher when included files change.
pub fn create_debouncer(
    kanata_arc: Arc<Mutex<Kanata>>,
    cfg_paths: &[PathBuf],
    included_files: &[PathBuf],
) -> Result<notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>> {
    // Create list of all files to watch
    let all_watched_files: Vec<PathBuf> = cfg_paths
        .iter()
        .chain(included_files.iter())
        .cloned()
        .collect();

    // Create debouncer with 500ms timeout and event handling closure
    let kanata_arc_clone = kanata_arc.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    for event in events {
                        // Check if the changed file is one of our watched files
                        if all_watched_files.iter().any(|watched_path| {
                            event.path.canonicalize().unwrap_or(event.path.clone())
                                == watched_path.canonicalize().unwrap_or(watched_path.clone())
                        }) {
                            match event.kind {
                                DebouncedEventKind::Any => {
                                    log::info!(
                                        "Config file changed: {}, triggering reload",
                                        event.path.display()
                                    );

                                    // Set the live_reload_requested flag
                                    if let Some(mut kanata) = kanata_arc_clone.try_lock() {
                                        kanata.request_live_reload();
                                    } else {
                                        log::warn!(
                                            "Could not acquire lock to set live_reload_requested"
                                        );
                                    }
                                }
                                _ => {
                                    log::trace!(
                                        "Ignoring file event: {:?} for {}",
                                        event.kind,
                                        event.path.display()
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("File watcher error: {:?}", e);
                }
            }
        },
    )?;

    // Watch all config files
    for path in cfg_paths {
        debouncer
            .watcher()
            .watch(path, RecursiveMode::NonRecursive)?;
        log::info!("Watching config file for changes: {}", path.display());
    }

    // Watch included files
    for path in included_files {
        debouncer
            .watcher()
            .watch(path, RecursiveMode::NonRecursive)?;
        log::info!("Watching included file for changes: {}", path.display());
    }

    Ok(debouncer)
}

pub fn restart_watcher(k_locked: &mut parking_lot::MutexGuard<Kanata>, k_ref: Arc<Mutex<Kanata>>) {
    log::info!("Restarting file watcher due to changes in included files");

    // Drop the old watcher. This is critical to stop its background thread
    // and release the file handles on the previously watched files.
    k_locked.file_watcher = None;

    // Create a new watcher with the updated file list
    let new_debouncer = match create_debouncer(k_ref, &k_locked.cfg_paths, &k_locked.included_files)
    {
        Ok(debouncer) => {
            log::info!("File watcher successfully restarted");
            Some(debouncer)
        }
        Err(e) => {
            log::error!("Failed to restart file watcher: {}", e);
            None
        }
    };

    k_locked.file_watcher = new_debouncer;
    k_locked.file_watcher_restart_requested = false;
}
