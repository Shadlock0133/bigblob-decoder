use std::{
    io::{self, Cursor, Read, Write},
    mem::size_of,
};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};

use crate::align_up;

pub fn create_dds_header(width: u32, height: u32) -> DdsHeader {
    let mipmap_count = calculate_mipmap_count(width, height);
    DdsHeader {
        height,
        width,
        pitch_or_linear_size: align_up::<4>(width) * align_up::<4>(height),
        depth: 0,
        mipmap_count,
        pixel_format: PixelFormat {
            four_cc: FourCC::DX10,
        },
        dx10_header: Some(Dx10Header {
            resource_dimension: ResourceDimension::Texture2D,
            alpha_mode: AlphaMode::Straight,
        }),
    }
}

pub fn calculate_mipmap_count(width: u32, height: u32) -> u32 {
    (32 - width.leading_zeros()).max(32 - height.leading_zeros())
}

pub fn parse_dds(data: &[u8]) -> Result<(DdsHeader, &[u8]), ParseError> {
    let mut cursor = Cursor::new(data);
    let header = DdsHeader::parse(&mut cursor)?;
    let offset = cursor.position() as usize;
    Ok((header, &data[offset..]))
}

#[derive(Debug)]
pub enum ParseError {
    Io(io::Error),
    WrongDDSMagic,
    WrongDDSHeaderSize,
    WrongPixelFormatSize,
    UnknownFourCC,
    UnknownFormat,
    UnknownResourceDimension,
    UnknownAlphaMode,
}

impl From<io::Error> for ParseError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
pub struct DdsHeader {
    pub height: u32,
    pub width: u32,
    pitch_or_linear_size: u32,
    depth: u32,
    pub mipmap_count: u32,
    pixel_format: PixelFormat,
    dx10_header: Option<Dx10Header>,
}
impl DdsHeader {
    const MAGIC: [u8; 4] = *b"DDS ";
    const SIZE: usize = 124;

    fn parse<R: Read>(mut r: R) -> Result<Self, ParseError> {
        if r.read_u32::<LE>()?.to_le_bytes() != Self::MAGIC {
            return Err(ParseError::WrongDDSMagic);
        }
        if r.read_u32::<LE>()? != Self::SIZE as u32 {
            return Err(ParseError::WrongDDSHeaderSize);
        }
        let _flags = r.read_u32::<LE>()?;
        let height = r.read_u32::<LE>()?;
        let width = r.read_u32::<LE>()?;
        let pitch_or_linear_size = r.read_u32::<LE>()?;
        let depth = r.read_u32::<LE>()?;
        let mipmap_count = r.read_u32::<LE>()?;
        // reserved1
        for _ in 0..11 {
            let _ = r.read_u32::<LE>()?;
        }
        let pixel_format = PixelFormat::parse(&mut r)?;
        // caps and reserved2
        for _ in 0..5 {
            let _ = r.read_u32::<LE>()?;
        }
        let dx10_header = matches!(pixel_format.four_cc, FourCC::DX10)
            .then(|| Dx10Header::parse(&mut r))
            .transpose()?;
        Ok(Self {
            height,
            width,
            pitch_or_linear_size,
            depth,
            mipmap_count,
            pixel_format,
            dx10_header,
        })
    }

    pub fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_all(&Self::MAGIC)?;
        // struct size
        w.write_u32::<LE>(Self::SIZE as u32)?;
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

enum FourCC {
    DX10,
}
impl FourCC {
    const DX10_BYTES: [u8; 4] = *b"DX10";
}

struct PixelFormat {
    four_cc: FourCC,
}
impl PixelFormat {
    const SIZE: usize = 8 * size_of::<u32>();

    fn parse<R: Read>(mut r: R) -> Result<Self, ParseError> {
        let size = r.read_u32::<LE>()?;
        if size != Self::SIZE as u32 {
            return Err(ParseError::WrongPixelFormatSize);
        }
        let _flags = r.read_u32::<LE>()?;
        let four_cc = match r.read_u32::<LE>()?.to_le_bytes() {
            FourCC::DX10_BYTES => FourCC::DX10,
            _ => return Err(ParseError::UnknownFourCC),
        };
        let _rgb_count = r.read_u32::<LE>()?;
        let _r_mask = r.read_u32::<LE>()?;
        let _g_mask = r.read_u32::<LE>()?;
        let _b_mask = r.read_u32::<LE>()?;
        let _a_mask = r.read_u32::<LE>()?;
        Ok(Self { four_cc })
    }

    fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        // struct size
        w.write_u32::<LE>(Self::SIZE as u32)?;
        let flags = 0x4; // DDPF_FOURCC
        w.write_u32::<LE>(flags)?;
        let four_cc = match self.four_cc {
            FourCC::DX10 => b"DX10",
        };
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
    const DXGI_FORMAT_BC7_UNORM: u32 = 98;

    fn parse<R: Read>(mut r: R) -> Result<Self, ParseError> {
        let format = r.read_u32::<LE>()?;
        if format != Self::DXGI_FORMAT_BC7_UNORM {
            return Err(ParseError::UnknownFormat);
        }
        let resource_dimension = match r.read_u32::<LE>()? {
            2 => ResourceDimension::Texture1D,
            3 => ResourceDimension::Texture2D,
            4 => ResourceDimension::Texture3D,
            _ => return Err(ParseError::UnknownResourceDimension),
        };
        let _misc = r.read_u32::<LE>()?;
        let _array_size = r.read_u32::<LE>()?;
        let alpha_mode = match r.read_u32::<LE>()? {
            0 => AlphaMode::Unknown,
            1 => AlphaMode::Straight,
            2 => AlphaMode::Premultiplied,
            3 => AlphaMode::Opaque,
            4 => AlphaMode::Custom,
            _ => return Err(ParseError::UnknownAlphaMode),
        };
        Ok(Self {
            resource_dimension,
            alpha_mode,
        })
    }

    fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_u32::<LE>(Self::DXGI_FORMAT_BC7_UNORM)?;
        w.write_u32::<LE>(self.resource_dimension as u32)?;
        // misc flag
        w.write_u32::<LE>(0)?;
        // array size
        w.write_u32::<LE>(1)?;
        w.write_u32::<LE>(self.alpha_mode as u32)?;
        Ok(())
    }
}
