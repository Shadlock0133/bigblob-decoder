mod dds;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use byteorder::{ReadBytesExt, LE};
use clap::Parser;
use image::{Rgba, RgbaImage};

pub fn align_up<const ALIGN: u32>(v: u32) -> u32 {
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

fn read_entry<R: Read>(r: &mut R) -> Result<Entry, io::Error> {
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

fn dump_entry(
    file: &mut File,
    entry: Entry,
    format: Format,
) -> Result<(), io::Error> {
    file.seek(SeekFrom::Start(entry.offset as _))?;
    let mut content = file.take(entry.size as _);
    let mut path = Path::new("dump").join(&entry.name);
    fs::create_dir_all(path.parent().unwrap())?;
    let content = {
        let mut buf = vec![];
        content.read_to_end(&mut buf)?;
        buf
    };
    let decompressed =
        lz4_flex::decompress(&content, entry.size_decompressed as _).unwrap();
    Ok(match (entry.file_type, format) {
        (FileType::Image, Format::Dds) => {
            path.set_extension("dds");
            let mut file = File::create(path)?;
            dds::create_dds_header(entry.width, entry.height)
                .write(&mut file)?;
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

fn decode_bc7(data: &[u8], width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let awidth = align_up::<4>(width);
    let aheight = align_up::<4>(height);
    let block_count = awidth * aheight / 16;
    let pos_iter = (0..aheight / 4)
        .flat_map(|y| (0..awidth / 4).map(move |x| (4 * x, 4 * y)));
    for (block, (x, y)) in data
        .chunks_exact(16)
        .map(|x| u128::from_le_bytes(x.try_into().unwrap()))
        .take(block_count as usize)
        .zip(pos_iter)
    {
        let pixels = decode_bc7_block(block).unwrap();
        for dy in 0..4 {
            for dx in 0..4 {
                if let Some(pixel) = image.get_pixel_mut_checked(x + dx, y + dy)
                {
                    *pixel = pixels[dy as usize][dx as usize];
                }
            }
        }
    }
    image
}

fn decode_bc7_block(block: u128) -> Result<[[Rgba<u8>; 4]; 4], ()> {
    // TODO: implement it
    if block == 0xaaaaaaac_00000000_00000020 {
        return Ok([[Rgba([0; 4]); 4]; 4]);
    }
    let mode = block.trailing_zeros();
    match mode {
        0 => return Ok([[Rgba([  0,   0,   0, 255]); 4]; 4]),
        1 => return Ok([[Rgba([  0,   0, 255, 255]); 4]; 4]),
        2 => return Ok([[Rgba([  0, 255,   0, 255]); 4]; 4]),
        3 => return Ok([[Rgba([  0, 255, 255, 255]); 4]; 4]),
        4 => return Ok([[Rgba([255,   0,   0, 255]); 4]; 4]),
        5 => return Ok([[Rgba([255,   0, 255, 255]); 4]; 4]),
        6 => return Ok([[Rgba([255, 255,   0, 255]); 4]; 4]),
        7 => return Ok([[Rgba([255, 255, 255, 255]); 4]; 4]),
        8.. => return Err(()),
    }
}

#[derive(Parser)]
struct Opt {
    #[clap(long)]
    image_format: Option<Format>,
    assets: Option<PathBuf>,
}

fn main() {
    let opts = Opt::parse();
    let filename = opts
        .assets
        .as_deref()
        .unwrap_or(Path::new("assets.bigblob"));
    let format = opts.image_format.unwrap_or(Format::Png);

    let mut file = File::open(filename).unwrap();
    let toc = read_toc(&mut file).unwrap();
    for entry in &toc.entries {
        print!(
            "{} ({:?}) ({} bytes @ {:#x}; {} decompressed)",
            entry.name,
            entry.file_type,
            entry.size,
            entry.offset,
            entry.size_decompressed
        );
        if entry.file_type == FileType::Image {
            println!(":");
            println!("dimensions: {}x{}", entry.width, entry.height);
            for unk in entry.unks {
                print!("({}, {}), ", unk.0, unk.1);
            }
        }
        println!();
    }
    dump_content(file, toc, format).unwrap();
}
