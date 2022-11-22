mod dds;

use std::{
    env,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
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
        let value = Entry {
            name,
            file_type,
            size,
            offset,
            size_decompressed,
            width,
            height,
            unks,
        };
        entries.push(value);
    }
    Ok(Toc { entries })
}

fn dump_content(mut file: File, toc: Toc) -> io::Result<()> {
    for entry in toc.entries {
        file.seek(SeekFrom::Start(entry.offset as _))?;
        let mut content = (&mut file).take(entry.size as _);
        let mut path = Path::new("dump").join(&entry.name);
        fs::create_dir_all(path.parent().unwrap())?;
        let content = {
            let mut buf = vec![];
            content.read_to_end(&mut buf)?;
            buf
        };
        let decompressed =
            lz4_flex::decompress(&content, entry.size_decompressed as _)
                .unwrap();
        if entry.file_type == FileType::Image {
            path.set_extension("dds");
        }
        let mut file = File::create(path)?;
        if entry.file_type == FileType::Image {
            dds::create_dds_header(entry.width, entry.height)
                .write(&mut file)?;
        }
        file.write_all(&decompressed)?;
    }
    Ok(())
}

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
            println!("dimensions: {}x{}", entry.width, entry.height);
            for unk in entry.unks {
                print!("({:#x}, {:#x}), ", unk.0, unk.1);
            }
            println!();
        }
    }
    dump_content(file, toc).unwrap();
}
