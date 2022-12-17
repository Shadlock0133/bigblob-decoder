use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(feature = "compressonator")]
use bigblob_decoder::bc7::encode_bc7_compressonator;
use bigblob_decoder::{
    bc7::encode_bc7,
    dds::{calculate_mipmap_count, create_dds_header, parse_dds},
    dump_content, dump_entry,
    encoding::{self, Archive, Data, Entry},
    read_toc, FileType, Format, Toc,
};
use clap::{Parser, ValueEnum};
use image::ImageFormat;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;

#[derive(Parser)]
struct ListContent {
    /// Location of "assets.bigblob" file
    assets: Option<PathBuf>,
}

#[derive(Parser)]
struct DumpContent {
    #[clap(long)]
    image_format: Option<Format>,
    /// Location of "assets.bigblob" file
    assets: Option<PathBuf>,
}

#[derive(Parser)]
struct DumpFile {
    #[clap(long)]
    image_format: Option<Format>,
    /// Location of "assets.bigblob" file
    assets: Option<PathBuf>,
    /// Name of an file inside assets to export
    entry_name: String,
}

#[derive(Clone, Copy, ValueEnum)]
enum Compressor {
    Internal,
    #[cfg(feature = "compressonator")]
    Compressonator,
}

#[derive(Parser)]
struct ReplaceEntry {
    /// Location of "assets.bigblob" file
    assets_input: Option<PathBuf>,
    assets_output: Option<PathBuf>,
    #[clap(long)]
    /// BC7 compressor for images
    compressor: Option<Compressor>,
    entry_name: String,
    file: PathBuf,
}

#[derive(Parser)]
struct ReplaceEntries {
    /// Location of "assets.bigblob" file
    assets_input: Option<PathBuf>,
    assets_output: Option<PathBuf>,
    #[clap(long)]
    /// BC7 compressor for images
    compressor: Option<Compressor>,
    folder: PathBuf,
}

#[derive(Parser)]
struct TestSetMetadata {
    /// Location of "assets.bigblob" file
    assets_input: Option<PathBuf>,
    assets_output: Option<PathBuf>,
    instructions: PathBuf,
}

#[derive(Parser)]
struct TestEncodeBc7 {
    input_image: PathBuf,
    output: PathBuf,
}

// TODO: make_archive
#[derive(Parser)]
enum Opt {
    ListContent(ListContent),
    ExtractAll(DumpContent),
    ExtractFile(DumpFile),
    ReplaceEntry(ReplaceEntry),
    ReplaceEntries(ReplaceEntries),
    TestSetMetadata(TestSetMetadata),
    TestEncodeBc7(TestEncodeBc7),
}

fn main() {
    let opts = Opt::parse();
    match opts {
        Opt::ListContent(opt) => list_content(opt),
        Opt::ExtractAll(opt) => extract_all(opt),
        Opt::ExtractFile(opt) => extract_file(opt),
        Opt::ReplaceEntry(opt) => replace_entry(opt),
        Opt::ReplaceEntries(opt) => replace_entries(opt),
        Opt::TestSetMetadata(opt) => test_set_metadata(opt),
        Opt::TestEncodeBc7(opt) => test_encode_bc7(opt),
    }
}

fn print_toc(toc: &Toc) {
    for entry in &toc.entries {
        println!(
            "{} ({:?}) ({} bytes @ {:#x}; {} decompressed)",
            entry.name,
            entry.file_type,
            entry.size,
            entry.offset,
            entry.size_decompressed
        );
        if entry.file_type == FileType::Image {
            println!("    dimensions: {}x{}", entry.width, entry.height);
            for (i, (x, y)) in entry.unks.iter().enumerate() {
                if (i, *x, *y) == (2, entry.width, entry.height) {
                    println!("    unk{i}: <same as dimensions>");
                } else {
                    println!("    unk{i}: {x}x{y}");
                }
            }
        }
    }
}

fn list_content(opts: ListContent) {
    let filename = opts
        .assets
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));

    let mut file = File::open(filename).unwrap();
    let toc = read_toc(&mut file).unwrap();
    print_toc(&toc);
}

fn extract_all(opts: DumpContent) {
    let filename = opts
        .assets
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));
    let format = opts.image_format.unwrap_or(Format::Png);

    let mut file = File::open(filename).unwrap();
    let toc = read_toc(&mut file).unwrap();
    dump_content(file, toc, format).unwrap();
}

fn extract_file(opts: DumpFile) {
    let filename = opts
        .assets
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));
    let format = opts.image_format.unwrap_or(Format::Png);

    let mut file = File::open(filename).unwrap();
    let toc = read_toc(&mut file).unwrap();
    let Some(entry) = toc.entries.into_iter().find(|e| e.name == opts.entry_name) else {
        panic!("Couldn't find file inside assets: {}", opts.entry_name);
    };
    dump_entry(&mut file, entry, format).unwrap();
}

fn replace_entry(opts: ReplaceEntry) {
    let assets_input_path = opts
        .assets_input
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));

    let mut assets_input = File::open(assets_input_path).unwrap();
    let toc = read_toc(&mut assets_input).unwrap();
    let mut archive = Archive::from_file_and_toc(&assets_input, toc).unwrap();
    drop(assets_input); // close the file

    let entry = archive
        .entries
        .iter_mut()
        .find(|e| e.name == opts.entry_name)
        .unwrap();
    replace_one_entry(entry, opts.file, opts.compressor);

    let output = opts.assets_output.as_deref().unwrap_or(assets_input_path);
    let assets_output = File::create(output).unwrap();
    archive.write_to_file(assets_output).unwrap();
}

fn replace_entries(opts: ReplaceEntries) {
    let assets_input_path = opts
        .assets_input
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));

    let mut assets_input = File::open(assets_input_path).unwrap();
    let toc = read_toc(&mut assets_input).unwrap();
    let mut archive = Archive::from_file_and_toc(&assets_input, toc).unwrap();
    drop(assets_input); // close the file

    let root = opts.folder.clone();

    let mut entries = archive.entries.iter_mut().collect::<Vec<_>>();
    let mut tasks = vec![];

    replace_entries_in_dir_rec(&mut entries, &mut tasks, &root, opts.folder)
        .unwrap();

    tasks.into_par_iter().for_each(|task| {
        println!("replacing {}", task.entry_name);
        replace_one_entry(task.entry, task.path, opts.compressor);
    });

    let output = opts.assets_output.as_deref().unwrap_or(assets_input_path);
    let assets_output = File::create(output).unwrap();
    archive.write_to_file(assets_output).unwrap();
}

struct Task<'a> {
    entry: &'a mut Entry,
    entry_name: String,
    path: PathBuf,
}

fn replace_entries_in_dir_rec<'a>(
    entries: &mut Vec<&'a mut Entry>,
    tasks: &mut Vec<Task<'a>>,
    root: &Path,
    path: PathBuf,
) -> std::io::Result<()> {
    for dir_entry in fs::read_dir(path)? {
        let dir_entry = dir_entry?;
        let file_type = dir_entry.file_type()?;
        if file_type.is_file() {
            let entry_path = dir_entry.path();
            let entry_path = entry_path.strip_prefix(root).unwrap();
            let entry_name = entry_path
                .components()
                .map(|c| c.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .unwrap()
                .join("/");
            let pos =
                entries.iter().position(|e| e.name == entry_name).unwrap();
            let entry = entries.remove(pos);
            tasks.push(Task {
                entry,
                entry_name,
                path: dir_entry.path(),
            });
        } else if file_type.is_dir() {
            replace_entries_in_dir_rec(entries, tasks, root, dir_entry.path())?;
        }
    }
    Ok(())
}

fn replace_one_entry(
    entry: &mut Entry,
    file: PathBuf,
    compressor: Option<Compressor>,
) {
    let mut data = fs::read(&file).unwrap();
    if file.extension() == Some(OsStr::new("png")) {
        let encoding::FileType::Image { width, height, .. } =
            &mut entry.file_type
        else {
            panic!("expected png file to replace \"Image\" file type entry")
        };
        let image =
            image::load_from_memory_with_format(&data, ImageFormat::Png)
                .unwrap()
                .into_rgba8();
        (*width, *height) = image.dimensions();

        let compressor = if let Some(c) = compressor {
            c
        } else {
            if cfg!(feature = "compressor") {
                panic!("missing compressor flag");
            } else {
                Compressor::Internal
            }
        };

        match compressor {
            Compressor::Internal => {
                eprintln!(
                    "Warning! internal compressor is currently WIP and \
                    only supports simple debug output"
                );
                data = encode_bc7(image);
            }
            #[cfg(feature = "compressonator")]
            Compressor::Compressonator => {
                data = encode_bc7_compressonator(image);
            }
        }
    } else if file.extension() == Some(OsStr::new("dds")) {
        match parse_dds(&data) {
            Ok((header, rest)) => {
                eprintln!("detected dds header, removing it");
                if header.mipmap_count
                    != calculate_mipmap_count(header.width, header.height)
                {
                    eprintln!(
                        "Warning! amount of mipmaps must be such that the \
                        smallest mipmap has size 1x1, otherwise the game will \
                        crash"
                    )
                }
                let encoding::FileType::Image { width, height, .. } =
                    &mut entry.file_type
                else {
                    panic!("expected dds file to replace \"Image\" file type entry")
                };
                *width = header.width;
                *height = header.height;
                data = rest.to_vec();
            }
            Err(e) => {
                eprintln!("failed parsing dds header: {e:?}");
                eprintln!("falling back to putting whole file");
            }
        }
    }
    entry.data = Data::Raw(data);
}

#[derive(Deserialize, Debug)]
struct Instruction {
    entry_name: String,
    offset_x: Option<u32>,
    offset_y: Option<u32>,
    double_offset: Option<bool>,
}

fn test_set_metadata(opts: TestSetMetadata) {
    let assets_input_path = opts
        .assets_input
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));

    let mut assets_input = File::open(assets_input_path).unwrap();
    let toc = read_toc(&mut assets_input).unwrap();
    let mut archive = Archive::from_file_and_toc(&assets_input, toc).unwrap();
    drop(assets_input); // close the file

    let instructions: Vec<Instruction> =
        serde_json::from_str(&fs::read_to_string(opts.instructions).unwrap())
            .unwrap();

    for instruction in instructions {
        let Some(entry) = archive
            .entries
            .iter_mut()
            .find(|e| e.name == instruction.entry_name)
        else {
            eprintln!("Couldn't find entry {:?}", instruction.entry_name);
            continue;
        };
        match &mut entry.file_type {
            encoding::FileType::Image { unks, .. } => {
                if let Some(offset_x) = instruction.offset_x {
                    unks[1].0 = offset_x;
                }
                if let Some(offset_y) = instruction.offset_y {
                    unks[1].1 = offset_y;
                }
                if let Some(true) = instruction.double_offset {
                    unks[1].0 *= 2;
                    unks[1].1 *= 2;
                }
            }
            _ => {
                eprintln!("entry {:?} is not an image", instruction.entry_name)
            }
        }
    }

    let output = opts.assets_output.as_deref().unwrap_or(assets_input_path);
    let assets_output = File::create(output).unwrap();
    archive.write_to_file(assets_output).unwrap();
}

fn test_encode_bc7(opts: TestEncodeBc7) {
    let image = image::open(opts.input_image).unwrap().into_rgba8();
    let (width, height) = image.dimensions();
    let contents = encode_bc7(image);
    let mut file = File::create(opts.output).unwrap();
    create_dds_header(width, height).write(&mut file).unwrap();
    file.write_all(&contents).unwrap();
}
