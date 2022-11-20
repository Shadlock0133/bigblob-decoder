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
        let path = Path::new("dump").join(entry.name);
        fs::create_dir_all(path.parent().unwrap())?;
        if entry.file_type == FileType::Sound {
            let decompressed = decompress_lz4(content)?;
            fs::write(path, decompressed)?;
        } else {
            io::copy(&mut content, &mut File::create(path)?)?;
        }
    }
    Ok(())
}

fn decompress_lz4<R: Read>(mut r: R) -> io::Result<Vec<u8>> {
    let mut res = vec![];
    loop {
        let token = r.read_u8()?;
        // read literals length
        let mut len = (token >> 4) as u64;
        if len == 15 {
            loop {
                let next_len_byte = r.read_u8()?;
                len += next_len_byte as u64;
                if next_len_byte != u8::MAX {
                    break;
                }
            }
        }
        // copy literals
        io::copy(&mut (&mut r).take(len), &mut res)?;
        // match copy
        // read back offset
        let offset = match r.read_u16::<LE>() {
            Ok(o) => o as usize,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        if offset == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // read matchlength
        let mut matchlen = (token & 0xf) as usize + 4;
        if matchlen == 19 {
            loop {
                let next_matchlen_byte = r.read_u8()?;
                matchlen += next_matchlen_byte as usize;
                if next_matchlen_byte != u8::MAX {
                    break;
                }
            }
        }
        // perform match copy
        let pos = res.len();
        let new_len = pos + matchlen;
        res.resize(new_len, 0);
        let start = pos - offset;
        let end = start + matchlen;
        res.copy_within(start..end, pos);
    }
    Ok(res)
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
            for unk in entry.unks {
                print!("{:#x?}, ", unk);
            }
            println!();
        }
    }
    dump_content(file, toc).unwrap();
}
