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
struct Block1 {
    #[deku(bits = 6)]
    partition: u8,
    #[deku(bits = 6)]
    r: [u8; 4],
    #[deku(bits = 6)]
    g: [u8; 4],
    #[deku(bits = 6)]
    b: [u8; 4],
    #[deku(bits = 1)]
    p: [u8; 2],
    #[deku(bits = 46)]
    index_data: u64,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block2 {
    #[deku(bits = 6)]
    partition: u8,
    #[deku(bits = 5)]
    r: [u8; 6],
    #[deku(bits = 5)]
    g: [u8; 6],
    #[deku(bits = 5)]
    b: [u8; 6],
    #[deku(bits = 29)]
    index_data: u64,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block3 {
    #[deku(bits = 6)]
    partition: u8,
    #[deku(bits = 7)]
    r: [u8; 4],
    #[deku(bits = 7)]
    g: [u8; 4],
    #[deku(bits = 7)]
    b: [u8; 4],
    #[deku(bits = 1)]
    p: [u8; 4],
    #[deku(bits = 30)]
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
    #[deku(bits = 31)]
    index_data0: u64,
    #[deku(bits = 47)]
    index_data1: u64,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block5 {
    rot: Rotation,
    #[deku(bits = 7)]
    r: [u8; 2],
    #[deku(bits = 7)]
    g: [u8; 2],
    #[deku(bits = 7)]
    b: [u8; 2],
    a: [u8; 2],
    #[deku(bits = 31)]
    colors: u64,
    #[deku(bits = 31)]
    alpha: u64,
}

#[derive(DekuRead, Debug)]
#[deku(endian = "little")]
struct Block6 {
    #[deku(bits = 7)]
    c0: [u8; 4],
    #[deku(bits = 7)]
    c1: [u8; 4],
    #[deku(bits = 1)]
    p: [u8; 2],
    #[deku(bits = 63)]
    index_data: u64,
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
    index_data: u64,
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

fn take_bits<const BITS: usize>(value: &mut u64) -> usize {
    let mask = (1 << BITS) - 1;
    let ret = *value & mask;
    *value >>= BITS;
    ret as usize
}

fn decode_bc7_block(block: u128) -> [[Rgba<u8>; 4]; 4] {
    // TODO: implement it
    // FIXME: output doesn't seem correct
    let mode = block.trailing_zeros();
    let le_bytes = block.to_le_bytes();
    let input = (&le_bytes[..], mode as usize + 1);
    match mode {
        0 => {
            let _data = Block0::from_bytes(input).unwrap().1;
            [[Rgba([0, 0, 0, 255]); 4]; 4]
        }
        1 => {
            let _data = Block1::from_bytes(input).unwrap().1;
            [[Rgba([0, 0, 255, 255]); 4]; 4]
        }
        2 => {
            let _data = Block2::from_bytes(input).unwrap().1;
            [[Rgba([0, 255, 0, 255]); 4]; 4]
        }
        3 => {
            let _data = Block3::from_bytes(input).unwrap().1;
            [[Rgba([0, 255, 255, 255]); 4]; 4]
        }
        4 => {
            let _data = Block4::from_bytes(input).unwrap().1;
            [[Rgba([255, 0, 0, 255]); 4]; 4]
        }
        5 => {
            let mut data = Block5::from_bytes(input).unwrap().1;

            let e0 = Rgb([data.r[0] << 1, data.g[0] << 1, data.b[0] << 1]);
            let e1 = Rgb([data.r[1] << 1, data.g[1] << 1, data.b[1] << 1]);
            let colors: [_; 4] = std::array::from_fn(|i| {
                e0.map2(&e1, |a, b| interpolate::<2>(a, b, i))
            });

            let alphas: [_; 4] = std::array::from_fn(|i| {
                interpolate::<2>(data.a[0], data.a[1], i)
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            for rgba in ret.iter_mut().flatten() {
                let [rgb @ .., a] = &mut rgba.0;
                *rgb = colors[take_bits::<2>(&mut data.colors)].0;
                *a = alphas[take_bits::<2>(&mut data.alpha)];
                data.rot.apply(rgba);
            }
            ret
        }
        6 => {
            let mut data = Block6::from_bytes(input).unwrap().1;

            let e0 = Rgba(data.c0).map(|x| x << 1 | data.p[0]);
            let e1 = Rgba(data.c1).map(|x| x << 1 | data.p[1]);
            let colors: [_; 16] = std::array::from_fn(|i| {
                e0.map2(&e1, |a, b| interpolate::<4>(a, b, i))
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            for rgba in ret.iter_mut().flatten() {
                *rgba = colors[take_bits::<4>(&mut data.index_data)];
            }

            ret
        }
        7 => {
            let _data = Block7::from_bytes(input).unwrap().1;
            [[Rgba([255, 255, 255, 255]); 4]; 4]
        }
        8.. => [[Rgba([0; 4]); 4]; 4],
    }
}

#[cfg(test)]
mod tests {
    use deku::DekuContainerRead;

    use crate::bc7::{
        Block0, Block1, Block2, Block3, Block4, Block5, Block6, Block7,
    };

    #[test]
    fn check_correct_block_size() {
        let block = 0u128;
        let le_bytes = block.to_le_bytes();
        let (rest, _data) = Block0::from_bytes((&le_bytes, 1)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block1::from_bytes((&le_bytes, 2)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block2::from_bytes((&le_bytes, 3)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        let (rest, _data) = Block3::from_bytes((&le_bytes, 4)).unwrap();
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
