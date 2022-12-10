pub mod bc7;
pub mod dds;
pub mod encoding;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    str::FromStr,
};

use bc7::decode_bc7;
use byteorder::{ReadBytesExt, LE};
use dds::create_dds_header;

pub const fn align_up<const ALIGN: u32>(v: u32) -> u32 {
    ((v + (ALIGN - 1)) / ALIGN) * ALIGN
}

#[derive(Debug)]
pub struct Toc {
    pub entries: Vec<DecodedEntry>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileType {
    Image = 0,
    Sound = 1,
    Unknown,
}

#[derive(Debug)]
pub struct DecodedEntry {
    pub name: String,
    pub file_type: FileType,
    pub size: u32,
    pub offset: u32,
    pub size_decompressed: u32,
    pub width: u32,
    pub height: u32,
    pub unks: [(u32, u32); 3],
}

pub fn read_toc<R: Read + Seek>(mut r: R) -> io::Result<Toc> {
    r.seek(SeekFrom::Start(0))?;
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

pub fn read_entry<R: Read>(r: &mut R) -> io::Result<DecodedEntry> {
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
    Ok(DecodedEntry {
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
pub enum Format {
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

pub fn dump_content(
    mut file: File,
    toc: Toc,
    format: Format,
) -> io::Result<()> {
    for entry in toc.entries {
        dump_entry(&mut file, entry, format)?;
    }
    Ok(())
}

pub fn dump_entry<R: Read + Seek>(
    mut file: R,
    entry: DecodedEntry,
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
