mod bc7;
mod dds;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use bc7::{decode_bc7, encode_bc7};
use byteorder::{ReadBytesExt, LE};
use clap::Parser;
use dds::create_dds_header;

pub const fn align_up<const ALIGN: u32>(v: u32) -> u32 {
    ((v + ALIGN - 1) / ALIGN) * ALIGN
}

#[derive(Debug)]
struct Toc {
    entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, Eq)]
enum FileType {
    Image = 0,
    Sound = 1,
    Unknown,
}

#[derive(Debug)]
struct Entry {
    name: String,
    file_type: FileType,
    size: u32,
    offset: u32,
    size_decompressed: u32,
    width: u32,
    height: u32,
    unks: [(u32, u32); 3],
}

fn read_toc<R: Read + Seek>(mut r: R) -> io::Result<Toc> {
    let toc_index = r.read_u32::<LE>()?;
    r.seek(SeekFrom::Start(toc_index as _))?;
    let entry_count = r.read_u32::<LE>()?;
    let mut entries = vec![];
    for _ in 0..entry_count {
        let entry = read_entry(&mut r)?;
        entries.push(entry);
    }
    Ok(Toc { entries })
}

fn read_entry<R: Read>(r: &mut R) -> io::Result<Entry> {
    let file_type = match r.read_u32::<LE>()? {
        0 => FileType::Image,
        1 => FileType::Sound,
        _ => FileType::Unknown,
    };
    let size_decompressed = r.read_u32::<LE>()?;
    let size = r.read_u32::<LE>()?;
    let unk2 = (r.read_u32::<LE>()?, r.read_u32::<LE>()?);
    let unk4 = (r.read_u32::<LE>()?, r.read_u32::<LE>()?);
    let unk6 = (r.read_u32::<LE>()?, r.read_u32::<LE>()?);
    let unks = [unk2, unk4, unk6];
    let width = r.read_u32::<LE>()?;
    let height = r.read_u32::<LE>()?;
    let offset = r.read_u32::<LE>()?;
    let name_len = r.read_u32::<LE>()?;
    let mut name_buf = vec![0; name_len as _];
    r.read_exact(&mut name_buf)?;
    let name = String::from_utf8(name_buf).unwrap();
    Ok(Entry {
        name,
        file_type,
        size,
        offset,
        size_decompressed,
        width,
        height,
        unks,
    })
}

#[derive(Clone, Copy)]
enum Format {
    Dds,
    Png,
}

impl FromStr for Format {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "dds" => Ok(Self::Dds),
            "png" => Ok(Self::Png),
            _ => Err("Invalid format"),
        }
    }
}

fn dump_content(mut file: File, toc: Toc, format: Format) -> io::Result<()> {
    for entry in toc.entries {
        dump_entry(&mut file, entry, format)?;
    }
    Ok(())
}

fn dump_entry<R: Read + Seek>(
    mut file: R,
    entry: Entry,
    format: Format,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(entry.offset as _))?;
    let mut file_section = file.take(entry.size as _);
    let mut path = Path::new("dump").join(&entry.name);
    fs::create_dir_all(path.parent().unwrap())?;
    let compressed = {
        let mut buf = vec![];
        file_section.read_to_end(&mut buf)?;
        buf
    };
    let decompressed =
        lz4_flex::decompress(&compressed, entry.size_decompressed as _)
            .unwrap();
    Ok(match (entry.file_type, format) {
        (FileType::Image, Format::Dds) => {
            path.set_extension("dds");
            let mut file = File::create(path)?;
            create_dds_header(entry.width, entry.height).write(&mut file)?;
            file.write_all(&decompressed)?;
        }
        (FileType::Image, Format::Png) => {
            decode_bc7(&decompressed, entry.width, entry.height)
                .save(&path)
                .unwrap();
        }
        (FileType::Sound | FileType::Unknown, _) => {
            fs::write(path, decompressed)?;
        }
    })
}

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
    file: String,
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
    TestEncodeBc7(TestEncodeBc7),
}

fn main() {
    let opts = Opt::parse();
    match opts {
        Opt::ListContent(opt) => list_content(opt),
        Opt::ExtractAll(opt) => extract_all(opt),
        Opt::ExtractFile(opt) => extract_file(opt),
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
    let Some(entry) = toc.entries.into_iter().find(|e| e.name == opts.file) else {
        eprintln!("Couldn't find file inside assets: {}", opts.file);
        return;
    };
    dump_entry(&mut file, entry, format).unwrap();
}

fn test_encode_bc7(opts: TestEncodeBc7) {
    let image = image::open(opts.input_image).unwrap().into_rgba8();
    let (width, height) = image.dimensions();
    let contents = encode_bc7(image);
    let mut file = File::create(opts.output).unwrap();
    create_dds_header(width, height).write(&mut file).unwrap();
    file.write_all(&contents).unwrap();
}
