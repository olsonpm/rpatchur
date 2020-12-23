mod patch_definition;

use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use clap::{App, Arg};
use gruf::thor::ThorArchiveBuilder;
use log::LevelFilter;
use patch_definition::{parse_patch_definition, PatchDefinition};
use simple_logger::SimpleLogger;
use walkdir::WalkDir;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

fn main() {
    // Parse CLI arguments
    let matches = app().get_matches();
    let verbose = matches.is_present("verbose");
    let patch_definition_file = PathBuf::from(
        matches
            .value_of("patch-definition")
            .expect("Missing positional argument"),
    );
    let data_directory = PathBuf::from(match matches.value_of("data-directory") {
        None => ".",
        Some(v) => v,
    });
    let output_path = matches.value_of("output");

    init_logger(verbose).expect("Failed to initalize the logger");
    // Parse YAML definition file
    log::info!("Processing '{}'", patch_definition_file.to_string_lossy());
    let patch_definition = parse_patch_definition(&patch_definition_file)
        .expect("Failed to parse the patch definition");

    // Display patch info
    log::info!("GRF merging: {}", patch_definition.use_grf_merging);
    log::info!("Checksums included: {}", patch_definition.include_checksums);
    if let Some(target_grf_name) = &patch_definition.target_grf_name {
        log::info!("Target GRF: '{}'", target_grf_name);
    }

    // Generate THOR archive
    let output_path = match output_path {
        None => PathBuf::from(
            patch_definition_file
                .with_extension("thor")
                .file_name()
                .expect("Invalid file name"),
        ),
        Some(v) => PathBuf::from(v),
    };
    let result = generate_patch_from_definition(patch_definition, data_directory, &output_path);
    match result {
        Err(e) => {
            log::error!("Failed to generate patch from definition: {}", e);
        }
        Ok(()) => {
            println!("Patch generated at '{}'", output_path.to_string_lossy());
        }
    }
}

fn app() -> App<'static, 'static> {
    App::new(PKG_NAME)
        .version(PKG_VERSION)
        .author(PKG_AUTHORS)
        .about(PKG_DESCRIPTION)
        .arg(
            Arg::with_name("patch-definition")
                .short("p")
                .long("patch-definition")
                .value_name("PATCH_DEFINITION")
                .help("Path to a patch definition file")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose logging"),
        )
        .arg(
            Arg::with_name("data-directory")
                .short("d")
                .long("data-directory")
                .value_name("DATA_DIRECTORY")
                .help("Path to the data directory")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("OUTPUT_FILE")
                .help("Path to the output archive")
                .takes_value(true),
        )
}

fn init_logger(verbose: bool) -> Result<()> {
    let level_filter = if verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Warn
    };

    SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level(PKG_NAME, level_filter)
        .init()?;
    Ok(())
}

fn generate_patch_from_definition<P1, P2>(
    patch_definition: PatchDefinition,
    data_directory: P1,
    output_path: P2,
) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let output_file = File::create(output_path)?;
    let mut archive_builder = ThorArchiveBuilder::new(
        output_file,
        patch_definition.use_grf_merging,
        patch_definition.target_grf_name,
        patch_definition.include_checksums,
    )?;
    for entry in patch_definition.entries {
        if entry.is_removed {
            log::trace!("'{}' will be REMOVED", &entry.relative_path);
            archive_builder.append_file_removal(entry.relative_path);
            continue;
        }

        let native_path = data_directory.as_ref().join(&entry.relative_path);
        if native_path.is_file() {
            // Path points to a single file
            log::trace!("'{}' will be UPDATED", &entry.relative_path);
            let file = File::open(native_path)?;
            archive_builder.append_file_update(entry.relative_path, file)?;
        } else if native_path.is_dir() {
            // Path points to a directory
            append_directory_update(&mut archive_builder, data_directory.as_ref(), native_path)?;
        } else {
            return Err(anyhow!(
                "Path '{}' is invalid or does not exist",
                native_path.to_string_lossy()
            ));
        }
    }
    Ok(())
}

fn append_directory_update<P1, P2>(
    archive_builder: &mut ThorArchiveBuilder<File>,
    data_directory: P1,
    directory_path: P2,
) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let walker = WalkDir::new(directory_path).follow_links(false).into_iter();
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel_path = entry.path().strip_prefix(data_directory.as_ref())?;
            let rel_path_str_lossy = rel_path.to_string_lossy();
            log::trace!("'{}' will be UPDATED", rel_path_str_lossy);
            let file = File::open(entry.path())?;
            archive_builder.append_file_update(rel_path_str_lossy.to_string(), file)?;
        }
    }
    Ok(())
}
