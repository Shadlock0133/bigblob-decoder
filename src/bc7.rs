use deku::{DekuContainerRead, DekuEnumExt, DekuError, DekuRead};
use image::{Pixel, Rgb, Rgba, RgbaImage};

use crate::align_up;

pub fn decode_bc7(data: &[u8], width: u32, height: u32) -> RgbaImage {
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
        let pixels = decode_bc7_block(block);
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

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block0 {
    #[deku(bits = 4)]
    partition: u8,
    #[deku(bits = 4)]
    r: [u8; 6],
    #[deku(bits = 4)]
    g: [u8; 6],
    #[deku(bits = 4)]
    b: [u8; 6],
    #[deku(bits = 1)]
    p: [u8; 6],
    #[deku(bits = 45)]
    index_data: u64,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block4 {
    rot: Rotation,
    #[deku(bits = 1)]
    idx_mode: bool,
    #[deku(bits = 5)]
    r: [u8; 2],
    #[deku(bits = 5)]
    g: [u8; 2],
    #[deku(bits = 5)]
    b: [u8; 2],
    #[deku(bits = 6)]
    a: [u8; 2],
    index_data0: ([Index2; 15], Index1),
    index_data1: ([Index3; 15], Index2),
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block5 {
    rot: Rotation,
    #[deku(bits = 7)]
    r0: u8,
    #[deku(bits = 7)]
    r1: u8,
    #[deku(bits = 7)]
    g0: u8,
    #[deku(bits = 7)]
    g1: u8,
    #[deku(bits = 7)]
    b0: u8,
    #[deku(bits = 7)]
    b1: u8,
    a0: u8,
    a1: u8,
    colors: ([Index2; 15], Index1),
    alpha: ([Index2; 15], Index1),
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block6 {
    #[deku(bits = 7)]
    r: [u8; 2],
    #[deku(bits = 7)]
    g: [u8; 2],
    #[deku(bits = 7)]
    b: [u8; 2],
    #[deku(bits = 7)]
    a: [u8; 2],
    #[deku(bits = 1)]
    p: [u8; 2],
    index_data: ([Index4; 15], Index3),
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block7 {
    #[deku(bits = 6)]
    partition: u8,
    #[deku(bits = 5)]
    r: [u8; 4],
    #[deku(bits = 5)]
    g: [u8; 4],
    #[deku(bits = 5)]
    b: [u8; 4],
    #[deku(bits = 5)]
    a: [u8; 4],
    #[deku(bits = 1)]
    p: [u8; 4],
    #[deku(bits = 30)]
    index_data: u32,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little", type = "u8", bits = 2, ctx = "_: deku::ctx::Endian")]
enum Rotation {
    No = 0,
    R = 1,
    G = 2,
    B = 3,
}

impl Rotation {
    fn apply(&self, color: &mut Rgba<u8>) {
        match self {
            Rotation::No => (),
            Rotation::R => color.0.swap(0, 3),
            Rotation::G => color.0.swap(1, 3),
            Rotation::B => color.0.swap(2, 3),
        }
    }
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little", type = "u8", bits = 1, ctx = "_: deku::ctx::Endian")]
enum Index1 {
    I0 = 0,
    I1 = 1,
}

impl From<Index1> for Index2 {
    fn from(i: Index1) -> Self {
        match i {
            Index1::I0 => Self::I0,
            Index1::I1 => Self::I1,
        }
    }
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little", type = "u8", bits = 2, ctx = "_: deku::ctx::Endian")]
enum Index2 {
    I0 = 0,
    I1 = 1,
    I2 = 2,
    I3 = 3,
}

impl From<Index2> for Index3 {
    fn from(i: Index2) -> Self {
        match i {
            Index2::I0 => Self::I0,
            Index2::I1 => Self::I1,
            Index2::I2 => Self::I2,
            Index2::I3 => Self::I3,
        }
    }
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little", type = "u8", bits = 3, ctx = "_: deku::ctx::Endian")]
enum Index3 {
    I0 = 0,
    I1 = 1,
    I2 = 2,
    I3 = 3,
    I4 = 4,
    I5 = 5,
    I6 = 6,
    I7 = 7,
}

impl From<Index3> for Index4 {
    fn from(i: Index3) -> Self {
        match i {
            Index3::I0 => Self::I0,
            Index3::I1 => Self::I1,
            Index3::I2 => Self::I2,
            Index3::I3 => Self::I3,
            Index3::I4 => Self::I4,
            Index3::I5 => Self::I5,
            Index3::I6 => Self::I6,
            Index3::I7 => Self::I7,
        }
    }
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little", type = "u8", bits = 4, ctx = "_: deku::ctx::Endian")]
enum Index4 {
    I0 = 0,
    I1 = 1,
    I2 = 2,
    I3 = 3,
    I4 = 4,
    I5 = 5,
    I6 = 6,
    I7 = 7,
    I8 = 8,
    I9 = 9,
    I10 = 10,
    I11 = 11,
    I12 = 12,
    I13 = 13,
    I14 = 14,
    I15 = 15,
}

const WEIGHT2: [u16; 4] = [0, 21, 43, 64];
const WEIGHT3: [u16; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHT4: [u16; 16] =
    [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
const WEIGHTS: [&[u16]; 3] = [&WEIGHT2, &WEIGHT3, &WEIGHT4];
fn interpolate<const BITS: usize>(a: u8, b: u8, index: usize) -> u8 {
    let da = (64 - WEIGHTS[BITS - 2][index]) * (a as u16);
    let db = WEIGHTS[BITS - 2][index] * (b as u16);
    ((da + db + 32) >> 6) as u8
}

fn decode_bc7_block(block: u128) -> [[Rgba<u8>; 4]; 4] {
    // TODO: implement it
    let mode = block.trailing_zeros();
    let le_bytes = block.to_le_bytes();
    let input = (&le_bytes[..], mode as usize + 1);
    match mode {
        0 => [[Rgba([0, 0, 0, 255]); 4]; 4],
        1 => [[Rgba([0, 0, 255, 255]); 4]; 4],
        2 => [[Rgba([0, 255, 0, 255]); 4]; 4],
        3 => [[Rgba([0, 255, 255, 255]); 4]; 4],
        4 => [[Rgba([255, 0, 0, 255]); 4]; 4],
        5 => {
            // FIXME: output doesn't seem correct
            let data = Block5::from_bytes(input).unwrap().1;

            let color0 = Rgb([data.r0 << 1, data.g0 << 1, data.b0 << 1]);
            let color1 = Rgb([data.r1 << 1, data.g1 << 1, data.b1 << 1]);
            let colors: [_; 4] = std::array::from_fn(|i| {
                color0.map2(&color1, |a, b| interpolate::<2>(a, b, i))
            });

            let alphas: [_; 4] =
                std::array::from_fn(|i| interpolate::<2>(data.a0, data.a1, i));

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            let color_indexes =
                data.colors.0.into_iter().chain([data.colors.1.into()]);
            let alpha_indexes =
                data.alpha.0.into_iter().chain([data.alpha.1.into()]);
            for ((rgba, ci), ai) in ret
                .iter_mut()
                .flatten()
                .zip(color_indexes)
                .zip(alpha_indexes)
            {
                let [rgb @ .., a] = &mut rgba.0;
                *rgb = colors[ci as usize].0;
                *a = alphas[ai as usize];
                data.rot.apply(rgba);
            }
            ret
        }
        6 => {
            let data = Block6::from_bytes(input).unwrap().1;

            let endpoint0 = Rgba([data.r[0], data.g[0], data.b[0], data.a[0]])
                .map(|x| x << 1 | data.p[0]);
            let endpoint1 = Rgba([data.r[1], data.g[1], data.b[1], data.a[1]])
                .map(|x| x << 1 | data.p[1]);
            let colors: [_; 16] = std::array::from_fn(|i| {
                endpoint0.map2(&endpoint1, |a, b| interpolate::<4>(a, b, i))
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            let indexes = data
                .index_data
                .0
                .into_iter()
                .chain([data.index_data.1.into()]);
            for (rgba, i) in ret.iter_mut().flatten().zip(indexes) {
                *rgba = colors[i as usize];
            }

            ret
        }
        7 => [[Rgba([255, 255, 255, 255]); 4]; 4],
        8.. => [[Rgba([0; 4]); 4]; 4],
    }
}

#[cfg(test)]
mod tests {
    use deku::DekuContainerRead;

    use crate::bc7::{Block0, Block4, Block5, Block6, Block7};

    #[test]
    fn check_correct_block_size() {
        let block = 0u128;
        let le_bytes = block.to_le_bytes();
        let (rest, _data) = Block0::from_bytes((&le_bytes, 1)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block4::from_bytes((&le_bytes, 5)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block5::from_bytes((&le_bytes, 6)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block6::from_bytes((&le_bytes, 7)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block7::from_bytes((&le_bytes, 8)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
    }
}
