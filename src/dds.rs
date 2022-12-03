use std::{
    io::{self, Write},
    mem::size_of,
};

use byteorder::{WriteBytesExt, LE};

use crate::align_up;

pub fn create_dds_header(width: u32, height: u32) -> DdsHeader {
    let mipmap_count =
        (32 - width.leading_zeros()).max(32 - height.leading_zeros());
    DdsHeader {
        height,
        width,
        pitch_or_linear_size: align_up::<4>(width) * align_up::<4>(height),
        depth: 0,
        mipmap_count,
        pixel_format: PixelFormat,
        dx10_header: Some(Dx10Header {
            resource_dimension: ResourceDimension::Texture2D,
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
        let magic = b"DDS ";
        w.write_all(magic)?;
        // struct size
        w.write_u32::<LE>(124)?;
        // flags
        let flags = 0x1 // DDSD_CAPS (required)
            | 0x2 // DDSD_HEIGHT (required)
            | 0x4 // DDSD_WIDTH (required)
            | 0x1000 // DDSD_PIXELFORMAT (required)
            | 0x2_0000 // DDSD_MIPMAPCOUNT
            | 0x8_0000; // DDSD_LINEARSIZE
        w.write_u32::<LE>(flags)?;
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
        let caps = 0x8 // DDSCAPS_COMPLEX (optional): more than one surface (e.g. a mipmap)
            | 0x40_0000 // DDSCAPS_MIPMAP (optional)
            | 0x1000; // DDSCAPS_TEXTURE (required)
        w.write_u32::<LE>(caps)?;
        // caps2: cubemap details/volume texture
        w.write_u32::<LE>(0)?;
        // caps3 (unused)
        w.write_u32::<LE>(0)?;
        // caps4 (unused)
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
        // struct size
        w.write_u32::<LE>(8 * size_of::<u32>() as u32)?;
        let flags = 0x4; // DDPF_FOURCC
        w.write_u32::<LE>(flags)?;
        let four_cc = b"DX10";
        w.write_all(four_cc)?;
        // rgb bit count
        w.write_u32::<LE>(0)?;
        // r mask
        w.write_u32::<LE>(0)?;
        // g mask
        w.write_u32::<LE>(0)?;
        // b mask
        w.write_u32::<LE>(0)?;
        // a mask
        w.write_u32::<LE>(0)?;
        Ok(())
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
enum ResourceDimension {
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
    resource_dimension: ResourceDimension,
    alpha_mode: AlphaMode,
}
impl Dx10Header {
    fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        let format = 98; // DXGI_FORMAT_BC7_UNORM
        w.write_u32::<LE>(format)?;
        w.write_u32::<LE>(self.resource_dimension as u32)?;
        // misc flag
        w.write_u32::<LE>(0)?;
        // array size
        w.write_u32::<LE>(1)?;
        w.write_u32::<LE>(self.alpha_mode as u32)?;
        Ok(())
    }
}
