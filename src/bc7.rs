use std::{
    array::from_fn,
    fmt::Debug,
    ops::{BitAnd, Shl, ShrAssign, Sub},
};

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

trait Decode {
    fn decode(block: u128) -> Self;
}

struct Block0 {
    partition: u8,
    r: [u8; 6],
    g: [u8; 6],
    b: [u8; 6],
    p: [u8; 6],
    index_data: u64,
}

struct Block1 {
    partition: u8,
    r: [u8; 4],
    g: [u8; 4],
    b: [u8; 4],
    p: [u8; 2],
    index_data: u64,
}

struct Block2 {
    partition: u8,
    r: [u8; 6],
    g: [u8; 6],
    b: [u8; 6],
    index_data: u64,
}

struct Block3 {
    partition: u8,
    r: [u8; 4],
    g: [u8; 4],
    b: [u8; 4],
    p: [u8; 4],
    index_data: u64,
}

struct Block4 {
    rot: Rotation,
    idx_mode: bool,
    r: [u8; 2],
    g: [u8; 2],
    b: [u8; 2],
    a: [u8; 2],
    index_data0: u64,
    index_data1: u64,
}

impl Decode for Block4 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 5>(&mut block);
        Self {
            rot: Rotation::from_u2(take_bits::<_, _, 2>(&mut block)),
            idx_mode: take_bits::<_, u8, 1>(&mut block) != 0,
            r: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            a: from_fn(|_| take_bits::<_, _, 6>(&mut block)),
            index_data0: take_bits::<_, _, 31>(&mut block),
            index_data1: take_bits::<_, _, 47>(&mut block),
        }
    }
}

struct Block5 {
    rot: Rotation,
    r: [u8; 2],
    g: [u8; 2],
    b: [u8; 2],
    a: [u8; 2],
    colors: u64,
    alpha: u64,
}

impl Decode for Block5 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 6>(&mut block);
        Self {
            rot: Rotation::from_u2(take_bits::<_, _, 2>(&mut block)),
            r: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            a: from_fn(|_| take_bits::<_, _, 8>(&mut block)),
            colors: take_bits::<_, _, 31>(&mut block),
            alpha: take_bits::<_, _, 31>(&mut block),
        }
    }
}

struct Block6 {
    r: [u8; 2],
    g: [u8; 2],
    b: [u8; 2],
    a: [u8; 2],
    p: [u8; 2],
    index_data: u64,
}

impl Decode for Block6 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 7>(&mut block);
        Self {
            r: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            a: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            p: from_fn(|_| take_bits::<_, _, 1>(&mut block)),
            index_data: take_bits::<_, _, 63>(&mut block),
        }
    }
}

struct Block7 {
    partition: u8,
    r: [u8; 4],
    g: [u8; 4],
    b: [u8; 4],
    a: [u8; 4],
    p: [u8; 4],
    index_data: u64,
}

#[derive(PartialEq, Debug)]
enum Rotation {
    No = 0,
    R = 1,
    G = 2,
    B = 3,
}

impl Rotation {
    fn from_u2(value: u8) -> Self {
        match value {
            0 => Self::No,
            1 => Self::R,
            2 => Self::G,
            3 => Self::B,
            _ => unreachable!(),
        }
    }

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

fn take_bits<
    T: From<u8>
        + Shl<usize, Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + ShrAssign<usize>
        + Copy,
    R: TryFrom<T>,
    const BITS: usize,
>(
    value: &mut T,
) -> R
where
    R::Error: Debug,
{
    let mask = (T::from(1) << BITS) - T::from(1);
    let ret = *value & mask;
    *value >>= BITS;
    R::try_from(ret).unwrap()
}

fn decode_bc7_block(block: u128) -> [[Rgba<u8>; 4]; 4] {
    // TODO: implement it
    let mode = block.trailing_zeros();
    match mode {
        0 => [[Rgba([0, 0, 0, 255]); 4]; 4],
        1 => [[Rgba([0, 0, 255, 255]); 4]; 4],
        2 => [[Rgba([0, 255, 0, 255]); 4]; 4],
        3 => [[Rgba([0, 255, 255, 255]); 4]; 4],
        4 => {
            let mut data = Block4::decode(block);

            let e = from_fn::<_, 2, _>(|i| {
                Rgb([data.r[i], data.g[i], data.b[i]])
                    .map(|x| x << 3)
                    .map(|x| x | x >> 5)
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            if data.idx_mode {
                // idx is 1, color index takes 3 bits
                let colors: [_; 8] = std::array::from_fn(|i| {
                    e[0].map2(&e[1], |a, b| interpolate::<3>(a, b, i))
                });
                let a = data.a.map(|x| x << 2).map(|x| x | x >> 6);
                let alphas: [_; 4] =
                    std::array::from_fn(|i| interpolate::<2>(a[0], a[1], i));

                for rgba in ret.iter_mut().flatten() {
                    let [rgb @ .., a] = &mut rgba.0;
                    *rgb = colors
                        [take_bits::<_, usize, 3>(&mut data.index_data1)]
                    .0;
                    *a =
                        alphas[take_bits::<_, usize, 2>(&mut data.index_data0)];
                    data.rot.apply(rgba);
                }
            } else {
                // idx is 0, color index takes 2 bits
                let colors: [_; 4] = std::array::from_fn(|i| {
                    e[0].map2(&e[1], |a, b| interpolate::<2>(a, b, i))
                });
                let alphas: [_; 8] = std::array::from_fn(|i| {
                    interpolate::<3>(data.a[0], data.a[1], i)
                });

                for rgba in ret.iter_mut().flatten() {
                    let [rgb @ .., a] = &mut rgba.0;
                    *rgb = colors
                        [take_bits::<_, usize, 2>(&mut data.index_data0)]
                    .0;
                    *a =
                        alphas[take_bits::<_, usize, 3>(&mut data.index_data1)];
                    data.rot.apply(rgba);
                }
            }

            ret
        }
        5 => {
            let mut data = Block5::decode(block);

            let e0 = Rgb([data.r[0], data.g[0], data.b[0]])
                .map(|x| x << 1)
                .map(|x| x | x >> 7);
            let e1 = Rgb([data.r[1], data.g[1], data.b[1]])
                .map(|x| x << 1)
                .map(|x| x | x >> 7);
            let colors: [_; 4] = std::array::from_fn(|i| {
                e0.map2(&e1, |a, b| interpolate::<2>(a, b, i))
            });

            let alphas: [_; 4] = std::array::from_fn(|i| {
                interpolate::<2>(data.a[0], data.a[1], i)
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            for rgba in ret.iter_mut().flatten() {
                let [rgb @ .., a] = &mut rgba.0;
                *rgb = colors[take_bits::<_, usize, 2>(&mut data.colors)].0;
                *a = alphas[take_bits::<_, usize, 2>(&mut data.alpha)];
                data.rot.apply(rgba);
            }
            ret
        }
        6 => {
            let mut data = Block6::decode(block);

            let e0 = Rgba([data.r[0], data.g[0], data.b[0], data.a[0]])
                .map(|x| x << 1 | data.p[0]);
            let e1 = Rgba([data.r[1], data.g[1], data.b[1], data.a[1]])
                .map(|x| x << 1 | data.p[1]);
            let colors: [_; 16] = std::array::from_fn(|i| {
                e0.map2(&e1, |a, b| interpolate::<4>(a, b, i))
            });

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            for rgba in ret.iter_mut().flatten() {
                *rgba = colors[take_bits::<_, usize, 4>(&mut data.index_data)];
            }

            ret
        }
        7 => [[Rgba([255, 255, 255, 255]); 4]; 4],
        8.. => [[Rgba([0; 4]); 4]; 4],
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use crate::bc7::{Block4, Decode};

    use super::{decode_bc7_block, Rotation};

    #[test]
    fn check_block4_max() {
        let block = u128::MAX;
        let data = Block4::decode(block);
        assert_eq!(data.rot, Rotation::B);
        assert_eq!(data.idx_mode, true);
        assert_eq!(data.r, [0b11111; 2]);
        assert_eq!(data.g, [0b11111; 2]);
        assert_eq!(data.b, [0b11111; 2]);
        assert_eq!(data.a, [0b111111; 2]);
        assert_eq!(data.index_data0, (1 << 31) - 1);
        assert_eq!(data.index_data1, (1 << 47) - 1);
    }

    #[test]
    fn check_block4_min() {
        let block = u128::MIN;
        let data = Block4::decode(block);
        assert_eq!(data.rot, Rotation::No);
        assert_eq!(data.idx_mode, false);
        assert_eq!(data.r, [0; 2]);
        assert_eq!(data.g, [0; 2]);
        assert_eq!(data.b, [0; 2]);
        assert_eq!(data.a, [0; 2]);
        assert_eq!(data.index_data0, 0);
        assert_eq!(data.index_data1, 0);
    }

    #[test]
    fn check_block4_test_content() {
        let block = 0b_101010_101010_11011_11011_11011_11011_11011_11011_0_10_10000_u128;
        let data = Block4::decode(block);
        assert_eq!(data.rot, Rotation::G);
        assert_eq!(data.idx_mode, false);
        assert_eq!(data.r, [0b11011; 2]);
        assert_eq!(data.g, [0b11011; 2]);
        assert_eq!(data.b, [0b11011; 2]);
        assert_eq!(data.a, [0b101010; 2]);
        assert_eq!(data.index_data0, 0);
        assert_eq!(data.index_data1, 0);
    }

    #[test]
    fn check_block4_content_by_bit() {
        for i in 0u8..128 {
            eprintln!("i: {i}");
            let block = 1u128 << i;
            let data = Block4::decode(block);
            match i {
                0..=4 => {
                    assert_eq!(data.rot, Rotation::No);
                    assert_eq!(data.idx_mode, false);
                    assert_eq!(data.r, [0; 2]);
                    assert_eq!(data.g, [0; 2]);
                    assert_eq!(data.b, [0; 2]);
                    assert_eq!(data.a, [0; 2]);
                    assert_eq!(data.index_data0, 0);
                    assert_eq!(data.index_data1, 0);
                }
                5 => assert_eq!(data.rot, Rotation::R),
                6 => assert_eq!(data.rot, Rotation::G),
                7 => assert_eq!(data.idx_mode, true),
                8..=12 => assert_eq!(data.r[0], 1 << (i - 8)),
                13..=17 => assert_eq!(data.r[1], 1 << (i - 13)),
                18..=22 => assert_eq!(data.g[0], 1 << (i - 18)),
                23..=27 => assert_eq!(data.g[1], 1 << (i - 23)),
                28..=32 => assert_eq!(data.b[0], 1 << (i - 28)),
                33..=37 => assert_eq!(data.b[1], 1 << (i - 33)),
                38..=43 => assert_eq!(data.a[0], 1 << (i - 38)),
                44..=49 => assert_eq!(data.a[1], 1 << (i - 44)),
                50..=80 => assert_eq!(data.index_data0, 1 << (i - 50)),
                81..=127 => assert_eq!(data.index_data1, 1 << (i - 81)),
                128.. => unreachable!(),
            }
        }
    }

    #[test]
    fn check_block8_decoding() {
        let output = decode_bc7_block(0);
        assert_eq!(output, [[Rgba([0; 4]); 4]; 4]);
    }

    #[test]
    fn check_transparent_decoding() {
        let output =
            decode_bc7_block(0x00000000_aaaaaaac_00000000_00000020_u128);
        assert_eq!(output, [[Rgba([0; 4]); 4]; 4]);
    }
}
