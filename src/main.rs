use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::{ReadBytesExt, LE};

#[derive(Debug)]
struct Toc {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    name: String,
    file_type: FileType,
    size: u32,
    offset: u32,
    unk0: u32,
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
        let unk0 = r.read_u32::<LE>()?;
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
            unk0,
            unks,
        };
        entries.push(value);
    }
    Ok(Toc { entries })
}

#[derive(Debug, PartialEq, Eq)]
enum FileType {
    Image = 0,
    Sound = 1,
    Unknown,
}

fn main() {
    let mut file = File::open("assets.bigblob").unwrap();
    let toc = read_toc(&mut file).unwrap();
    // for typ in [FileType::Sound, FileType::Image] {
    //     let maps = [(); 8].map(|()| BTreeMap::<u32, usize>::new());
    //     let occs = toc.entries.iter().filter(|e| e.file_type == typ).fold(
    //         maps,
    //         |mut acc, e| {
    //             for (map, e) in acc.iter_mut().zip(e.unks) {
    //                 *map.entry(e).or_default() += 1;
    //             }
    //             acc
    //         },
    //     );
    //     eprintln!("typ: {typ:?}");
    //     for (i, occ) in occs.iter().enumerate() {
    //         if typ == FileType::Sound {
    //             eprintln!("[{i}]: {occ:?}");
    //         }
    //         eprintln!("[{i}]: {}", occ.len());
    //     }
    // }
    for entry in &toc.entries {
        print!(
            "{} ({:?}) ({:#x} bytes @ {:#x}):",
            entry.name, entry.file_type, entry.size, entry.offset
        );
        println!(" {:#010x?}", entry.unk0);
        if entry.file_type == FileType::Image {
            for unk in entry.unks {
                print!("{:#x?}, ", unk);
            }
            println!();
        }
    }
    dump_content(file, toc).unwrap();
}

fn dump_content(mut file: File, toc: Toc) -> io::Result<()> {
    for entry in toc.entries {
        file.seek(SeekFrom::Start(entry.offset as _))?;
        let mut content = file.by_ref().take(entry.size as _);
        let path = Path::new("dump").join(entry.name);
        fs::create_dir_all(path.parent().unwrap())?;
        io::copy(&mut content, &mut File::create(path)?)?;
    }
    Ok(())
}
