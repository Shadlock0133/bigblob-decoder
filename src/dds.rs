#![allow(dead_code)]

use std::{
    io::{self, Write},
    mem::size_of,
};

use byteorder::{WriteBytesExt, LE};

pub fn create_dds_header(width: u32, height: u32) -> DdsHeader {
    DdsHeader {
        height,
        width,
        pitch_or_linear_size: 0x228000,
        depth: 1,
        mipmap_count: 11,
        pixel_format: PixelFormat,
        dx10_header: Some(Dx10Header {
            resource_dimension: ResDim::Texture2D,
            alpha_mode: AlphaMode::Straight,
        }),
    }
}

/// https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
pub struct DdsHeader {
    height: u32,
    width: u32,
    pitch_or_linear_size: u32,
    depth: u32,
    mipmap_count: u32,
    pixel_format: PixelFormat,
    dx10_header: Option<Dx10Header>,
}
impl DdsHeader {
    pub fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_all(b"DDS ")?;
        w.write_u32::<LE>(124)?;
        w.write_u32::<LE>(0x000a1007)?;
        w.write_u32::<LE>(self.height)?;
        w.write_u32::<LE>(self.width)?;
        w.write_u32::<LE>(self.pitch_or_linear_size)?;
        w.write_u32::<LE>(self.depth)?;
        w.write_u32::<LE>(self.mipmap_count)?;
        // reserved1
        for _ in 0..11 {
            w.write_u32::<LE>(0)?;
        }
        self.pixel_format.write(&mut w)?;
        w.write_u32::<LE>(0x00401008)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        // reserved2
        w.write_u32::<LE>(0)?;
        if let Some(header) = &self.dx10_header {
            header.write(&mut w)?;
        }
        Ok(())
    }
}

struct PixelFormat;
impl PixelFormat {
    fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_u32::<LE>(8 * size_of::<u32>() as u32)?;
        w.write_u32::<LE>(4)?;
        w.write_all(b"DX10")?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(0)?;
        Ok(())
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
enum ResDim {
    Texture1D = 2,
    Texture2D = 3,
    Texture3D = 4,
}
#[repr(u32)]
#[derive(Clone, Copy)]
enum AlphaMode {
    Unknown = 0,
    Straight = 1,
    Premultiplied = 2,
    Opaque = 3,
    Custom = 4,
}

struct Dx10Header {
    resource_dimension: ResDim,
    alpha_mode: AlphaMode,
}
impl Dx10Header {
    fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_u32::<LE>(98)?;
        w.write_u32::<LE>(self.resource_dimension as u32)?;
        w.write_u32::<LE>(0)?;
        w.write_u32::<LE>(1)?;
        w.write_u32::<LE>(1)?;
        Ok(())
    }
}
