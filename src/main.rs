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
    encoding::{self, Archive, Data},
    read_toc, FileType, Format, Toc,
};
use clap::{Parser, ValueEnum};
use image::ImageFormat;

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

#[derive(Clone, ValueEnum)]
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
struct TestEncodeBc7 {
    input_image: PathBuf,
    output: PathBuf,
}

#[derive(Parser)]
enum Opt {
    ListContent(ListContent),
    ExtractAll(DumpContent),
    ExtractFile(DumpFile),
    ReplaceEntry(ReplaceEntry),
    TestEncodeBc7(TestEncodeBc7),
}

fn main() {
    let opts = Opt::parse();
    match opts {
        Opt::ListContent(opt) => list_content(opt),
        Opt::ExtractAll(opt) => extract_all(opt),
        Opt::ExtractFile(opt) => extract_file(opt),
        Opt::ReplaceEntry(opt) => replace_entry(opt),
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
        eprintln!("Couldn't find file inside assets: {}", opts.entry_name);
        return;
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
    let entry = archive
        .entries
        .iter_mut()
        .find(|e| e.name == opts.entry_name)
        .unwrap();
    let mut data = fs::read(&opts.file).unwrap();

    if opts.file.extension() == Some(OsStr::new("png")) {
        // panic!("png format is currently unsupported, use some other program to \
        //     compress it into dds file with bc7 format texture");
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

        let compressor = if let Some(c) = opts.compressor {
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
    } else if opts.file.extension() == Some(OsStr::new("dds")) {
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
    drop(assets_input); // close the file

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
