use std::{
    env,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::{ReadBytesExt, LE};

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
    unks: [u32; 8],
}

fn read_toc<R: Read + Seek>(mut r: R) -> io::Result<Toc> {
    let toc_index = r.read_u32::<LE>()?;
    r.seek(SeekFrom::Start(toc_index as _))?;
    let entry_count = r.read_u32::<LE>()?;
    let mut entries = vec![];
    for _ in 0..entry_count {
        let file_type = match r.read_u32::<LE>()? {
            0 => FileType::Image,
            1 => FileType::Sound,
            _ => FileType::Unknown,
        };
        let size_decompressed = r.read_u32::<LE>()?;
        let size = r.read_u32::<LE>()?;
        let unks = [(); 8].map(|()| r.read_u32::<LE>().unwrap());
        let offset = r.read_u32::<LE>()?;
        let name_len = r.read_u32::<LE>()?;
        let mut name_buf = vec![0; name_len as _];
        r.read_exact(&mut name_buf)?;
        let name = String::from_utf8(name_buf).unwrap();
        let value = Entry {
            name,
            file_type,
            size,
            offset,
            size_decompressed,
            unks,
        };
        entries.push(value);
    }
    Ok(Toc { entries })
}

fn dump_content(mut file: File, toc: Toc) -> io::Result<()> {
    for entry in toc.entries {
        file.seek(SeekFrom::Start(entry.offset as _))?;
        let mut content = file.by_ref().take(entry.size as _);
        let mut path = Path::new("dump").join(&entry.name);
        fs::create_dir_all(path.parent().unwrap())?;
        let content = {
            let mut buf = vec![];
            content.read_to_end(&mut buf)?;
            buf
        };
        let mut decompressed =
            lz4_flex::decompress(&content, entry.size_decompressed as _)
                .unwrap();
        if entry.file_type == FileType::Image {
            path.set_extension("dds");
            decompressed = DDS_PREFIX_0
                .iter()
                .copied()
                .chain(entry.unks[7].to_le_bytes())
                .chain(entry.unks[6].to_le_bytes())
                .chain(DDS_PREFIX_1.iter().copied())
                .chain(decompressed)
                .collect();
        }
        fs::write(path, decompressed)?;
    }
    Ok(())
}

const DDS_PREFIX_0: &[u8] = &[
    0x44, 0x44, 0x53, 0x20, 0x7C, 0x00, 0x00, 0x00, 0x07, 0x10, 0x0A, 0x00,
];

const DDS_PREFIX_1: &[u8] = &[
    0x00, 0x80, 0x22, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0B, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00, 0x44, 0x58, 0x31, 0x30, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x08, 0x10, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x62, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
];

fn main() {
    let arg1 = env::args().nth(1);
    let filename = arg1.as_deref().unwrap_or("assets.bigblob");
    let mut file = File::open(filename).unwrap();
    let toc = read_toc(&mut file).unwrap();
    for entry in &toc.entries {
        println!(
            "{} ({:?}) ({} bytes @ {:#x}; {} decompressed):",
            entry.name,
            entry.file_type,
            entry.size,
            entry.offset,
            entry.size_decompressed
        );
        if entry.file_type == FileType::Image {
            for unk in entry.unks {
                print!("{:#x?}, ", unk);
            }
            println!();
        }
    }
    dump_content(file, toc).unwrap();
}
