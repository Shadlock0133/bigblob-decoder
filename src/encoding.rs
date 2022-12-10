use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    mem::size_of,
};

use byteorder::{WriteBytesExt, LE};

use crate::Toc;

pub enum FileType {
    Image {
        width: u32,
        height: u32,
        unks: [(u32, u32); 3],
    },
    Sound,
}

pub enum Data {
    Compressed {
        data: Vec<u8>,
        uncompressed_size: u32,
    },
    Raw(Vec<u8>),
}

pub struct Entry {
    pub name: String,
    pub file_type: FileType,
    pub data: Data,
}

struct CompressedEntry {
    name: String,
    file_type: FileType,
    data: Vec<u8>,
    uncompressed_size: u32,
}

struct WrittenEntry {
    name: String,
    file_type: FileType,
    uncompressed_size: u32,
    size: u32,
    offset: u32,
}

pub struct Archive {
    pub entries: Vec<Entry>,
}

impl Archive {
    pub fn from_file_and_toc<R: Read + Seek>(
        mut file: R,
        toc: Toc,
    ) -> io::Result<Self> {
        let mut entries = vec![];
        for entry in toc.entries {
            let file_type = match entry.file_type {
                crate::FileType::Image => FileType::Image {
                    width: entry.width,
                    height: entry.height,
                    unks: entry.unks,
                },
                crate::FileType::Sound => FileType::Sound,
                crate::FileType::Unknown => unimplemented!(),
            };
            file.seek(SeekFrom::Start(entry.offset as _))?;
            let mut file_section = (&mut file).take(entry.size as _);
            let data = {
                let mut buf = vec![];
                file_section.read_to_end(&mut buf)?;
                buf
            };
            entries.push(Entry {
                name: entry.name,
                file_type,
                data: Data::Compressed {
                    data,
                    uncompressed_size: entry.size_decompressed,
                },
            });
        }
        Ok(Self { entries })
    }

    pub fn write_to_file<W: Write>(self, mut w: W) -> io::Result<()> {
        let compressed_entries: Vec<_> = self
            .entries
            .into_iter()
            .map(|e| {
                let (data, uncompressed_size) = match e.data {
                    Data::Compressed {
                        data,
                        uncompressed_size,
                    } => (data, uncompressed_size),
                    Data::Raw(d) => (lz4_flex::compress(&d), d.len() as u32),
                };
                CompressedEntry {
                    name: e.name,
                    file_type: e.file_type,
                    data,
                    uncompressed_size,
                }
            })
            .collect();
        let data_size: u32 = compressed_entries
            .iter()
            .map(|e| e.data.len())
            .sum::<usize>()
            .try_into()
            .unwrap();
        let start_of_toc = data_size + size_of::<u32>() as u32;
        w.write_u32::<LE>(start_of_toc)?;
        // write data
        let mut running_offset = size_of::<u32>() as u32;
        let written_entries = compressed_entries
            .into_iter()
            .map(|e| {
                let size = e.data.len() as u32;
                let offset = running_offset;
                running_offset += size;
                w.write_all(&e.data)?;
                Ok(WrittenEntry {
                    name: e.name,
                    file_type: e.file_type,
                    uncompressed_size: e.uncompressed_size,
                    size,
                    offset,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        // write toc
        w.write_u32::<LE>(written_entries.len() as u32)?;
        for entry in written_entries {
            let (file_type_tag, width, height, unks) = match entry.file_type {
                FileType::Image {
                    width,
                    height,
                    unks,
                } => (0, width, height, unks),
                FileType::Sound => (1, 0, 0, [(0, 0); 3]),
            };
            w.write_u32::<LE>(file_type_tag)?;
            w.write_u32::<LE>(entry.uncompressed_size)?;
            w.write_u32::<LE>(entry.size)?;
            for (x, y) in unks {
                w.write_u32::<LE>(x)?;
                w.write_u32::<LE>(y)?;
            }
            w.write_u32::<LE>(width)?;
            w.write_u32::<LE>(height)?;
            w.write_u32::<LE>(entry.offset)?;
            w.write_u32::<LE>(entry.name.len() as u32)?;
            w.write_all(entry.name.as_bytes())?;
        }
        Ok(())
    }
}
