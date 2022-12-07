use std::{
    array::from_fn,
    fmt::Debug,
    mem::size_of,
    ops::{BitAnd, Shl, ShrAssign, Sub},
};

use image::{Pixel, Rgb, Rgba, RgbaImage};

use crate::align_up;

use super::{
    interpolate, Block0, Block1, Block2, Block3, Block4, Block5, Block6,
    Block7, Rotation, ANCHOR_INDEX_2, PARTITIONS_2, PARTITIONS_3,
};

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

pub fn decode_bc7_block(block: u128) -> [[Rgba<u8>; 4]; 4] {
    // FIXME: output doesn't match
    // TODO: anchors
    let mode = block.trailing_zeros();
    match mode {
        0 => {
            let data = Block0::decode(block);

            let subsets: [[Rgb<u8>; 8]; 3] = from_fn(|sub| {
                let e: [_; 2] = from_fn(|i| {
                    let index = 2 * sub + i;
                    Rgb([data.r[index], data.g[index], data.b[index]])
                        .map(|x| (x << 1) | data.p[index])
                        .map(|x| x << (8 - 5))
                        .map(|x| x | x >> 5)
                });
                from_fn(|i| e[0].map2(&e[1], |a, b| interpolate::<3>(a, b, i)))
            });

            let mut ret = [[Rgba([0, 0, 0, 255]); 4]; 4];
            let mut index_data = data.index_data;
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let [rgb @ .., _] = &mut rgba.0;
                let subset = PARTITIONS_3[data.partition as usize][i];
                let index = take_bits::<_, usize, 2>(&mut index_data);
                *rgb = subsets[subset][index].0;
            }
            ret
        }
        1 => {
            let data = Block1::decode(block);

            let subsets: [[Rgb<u8>; 8]; 2] = from_fn(|sub| {
                let e: [_; 2] = from_fn(|i| {
                    let index = 2 * sub + i;
                    Rgb([data.r[index], data.g[index], data.b[index]])
                        .map(|x| (x << 1) | data.p[sub])
                        .map(|x| x << (8 - 7))
                        .map(|x| x | x >> 7)
                });
                from_fn(|i| e[0].map2(&e[1], |a, b| interpolate::<3>(a, b, i)))
            });

            let mut ret = [[Rgba([0, 0, 0, 255]); 4]; 4];
            let mut index_data = data.index_data;
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let [rgb @ .., _] = &mut rgba.0;
                let subset = PARTITIONS_2[data.partition as usize][i];
                let index = take_bits::<_, usize, 3>(&mut index_data);
                *rgb = subsets[subset][index].0;
            }
            ret
        }
        2 => {
            let data = Block2::decode(block);

            let subsets: [[Rgb<u8>; 4]; 2] = from_fn(|sub| {
                let e: [_; 2] = from_fn(|i| {
                    let index = 2 * sub + i;
                    Rgb([data.r[index], data.g[index], data.b[index]])
                        .map(|x| x << (8 - 5))
                        .map(|x| x | x >> 5)
                });
                from_fn(|i| e[0].map2(&e[1], |a, b| interpolate::<2>(a, b, i)))
            });

            let mut ret = [[Rgba([0, 0, 0, 255]); 4]; 4];
            let mut index_data = data.index_data;
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let [rgb @ .., _] = &mut rgba.0;
                let subset = PARTITIONS_3[data.partition as usize][i];
                let index = take_bits::<_, usize, 2>(&mut index_data);
                *rgb = subsets[subset][index].0;
            }
            ret
        }
        3 => {
            let data = Block3::decode(block);

            let subsets: [[Rgb<u8>; 8]; 3] = from_fn(|sub| {
                let e: [_; 2] = from_fn(|i| {
                    let index = 2 * sub + i;
                    Rgb([data.r[index], data.g[index], data.b[index]])
                        .map(|x| (x << 1) | data.p[index])
                });
                from_fn(|i| e[0].map2(&e[1], |a, b| interpolate::<3>(a, b, i)))
            });

            let mut ret = [[Rgba([0, 0, 0, 255]); 4]; 4];
            let mut index_data = data.index_data;
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let [rgb @ .., _] = &mut rgba.0;
                let subset = PARTITIONS_2[data.partition as usize][i];
                let index = take_bits::<_, usize, 2>(&mut index_data);
                *rgb = subsets[subset][index].0;
            }
            ret
        }
        4 => {
            // TODO: fix
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

                for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                    let [rgb @ .., a] = &mut rgba.0;
                    let (color_index, alpha_index) = if i == 0 {
                        (
                            take_bits::<_, usize, 2>(&mut data.index_data1),
                            take_bits::<_, usize, 1>(&mut data.index_data0),
                        )
                    } else {
                        (
                            take_bits::<_, usize, 3>(&mut data.index_data1),
                            take_bits::<_, usize, 2>(&mut data.index_data0),
                        )
                    };
                    *rgb = colors[color_index].0;
                    *a = alphas[alpha_index];
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

                for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                    let [rgb @ .., a] = &mut rgba.0;
                    let (color_index, alpha_index) = if i == 0 {
                        (
                            take_bits::<_, usize, 1>(&mut data.index_data0),
                            take_bits::<_, usize, 2>(&mut data.index_data1),
                        )
                    } else {
                        (
                            take_bits::<_, usize, 2>(&mut data.index_data0),
                            take_bits::<_, usize, 3>(&mut data.index_data1),
                        )
                    };
                    *rgb = colors[color_index].0;
                    *a = alphas[alpha_index];
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
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let [rgb @ .., a] = &mut rgba.0;
                let (color_index, alpha_index) = if i == 0 {
                    (
                        take_bits::<_, usize, 1>(&mut data.colors),
                        take_bits::<_, usize, 1>(&mut data.alpha),
                    )
                } else {
                    (
                        take_bits::<_, usize, 2>(&mut data.colors),
                        take_bits::<_, usize, 2>(&mut data.alpha),
                    )
                };
                *rgb = colors[color_index].0;
                *a = alphas[alpha_index];
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
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let index = if i == 0 {
                    take_bits::<_, usize, 3>(&mut data.index_data)
                } else {
                    take_bits::<_, usize, 4>(&mut data.index_data)
                };
                *rgba = colors[index];
            }

            ret
        }
        7 => {
            let data = Block7::decode(block);

            let subsets: [[Rgba<u8>; 4]; 2] = from_fn(|sub| {
                let e: [_; 2] = from_fn(|i| {
                    let index = 2 * sub + i;
                    Rgba([
                        data.r[index],
                        data.g[index],
                        data.b[index],
                        data.a[index],
                    ])
                    .map(|x| (x << 1) | data.p[index])
                    .map(|x| x << (8 - 6))
                    .map(|x| x | x >> 6)
                });
                from_fn(|i| e[0].map2(&e[1], |a, b| interpolate::<2>(a, b, i)))
            });

            let mut ret = [[Rgba([0, 0, 0, 255]); 4]; 4];
            let mut index_data = data.index_data;
            for (i, rgba) in ret.iter_mut().flatten().enumerate() {
                let subset = PARTITIONS_2[data.partition as usize][i];
                let index = if i == 0 || i == ANCHOR_INDEX_2[subset] {
                    take_bits::<_, usize, 1>(&mut index_data)
                } else {
                    take_bits::<_, usize, 2>(&mut index_data)
                };
                *rgba = subsets[subset][index];
            }
            ret
        }
        8.. => [[Rgba([0; 4]); 4]; 4],
    }
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
    const ERR_MSG: &str = "BITS must be between 0 and size of T in bits";
    assert!(0 < BITS, "{}", ERR_MSG);
    assert!(BITS <= (8 * size_of::<T>()), "{}", ERR_MSG);
    assert!(BITS <= (8 * size_of::<R>()), "{}", ERR_MSG);

    let mask = (T::from(1) << BITS) - T::from(1);
    let ret = *value & mask;
    *value >>= BITS;
    R::try_from(ret).unwrap()
}

trait Decode {
    fn decode(block: u128) -> Self;
}

impl Decode for Block0 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 1>(&mut block);
        Self {
            partition: take_bits::<_, u8, 4>(&mut block),
            r: from_fn(|_| take_bits::<_, _, 4>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 4>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 4>(&mut block)),
            p: from_fn(|_| take_bits::<_, _, 1>(&mut block)),
            index_data: take_bits::<_, _, 45>(&mut block),
        }
    }
}

impl Decode for Block1 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 2>(&mut block);
        Self {
            partition: take_bits::<_, u8, 6>(&mut block),
            r: from_fn(|_| take_bits::<_, _, 6>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 6>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 6>(&mut block)),
            p: from_fn(|_| take_bits::<_, _, 1>(&mut block)),
            index_data: take_bits::<_, _, 46>(&mut block),
        }
    }
}

impl Decode for Block2 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 3>(&mut block);
        Self {
            partition: take_bits::<_, u8, 6>(&mut block),
            r: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            index_data: take_bits::<_, _, 29>(&mut block),
        }
    }
}

impl Decode for Block3 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 4>(&mut block);
        Self {
            partition: take_bits::<_, u8, 6>(&mut block),
            r: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 7>(&mut block)),
            p: from_fn(|_| take_bits::<_, _, 1>(&mut block)),
            index_data: take_bits::<_, _, 30>(&mut block),
        }
    }
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

impl Decode for Block7 {
    fn decode(mut block: u128) -> Self {
        let _mode: u8 = take_bits::<_, _, 8>(&mut block);
        Self {
            partition: take_bits::<_, _, 6>(&mut block),
            r: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            g: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            b: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            a: from_fn(|_| take_bits::<_, _, 5>(&mut block)),
            p: from_fn(|_| take_bits::<_, _, 1>(&mut block)),
            index_data: take_bits::<_, _, 30>(&mut block),
        }
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use crate::bc7::{
        decode::{decode_bc7_block, Decode},
        Block0, Block1, Block2, Block3, Block4, Block5, Block6, Block7,
        Rotation,
    };

    const B1: u8 = (1 << 1) - 1;
    const B2: u8 = (1 << 2) - 1;
    const B3: u8 = (1 << 3) - 1;
    const B4: u8 = (1 << 4) - 1;
    const B5: u8 = (1 << 5) - 1;
    const B6: u8 = (1 << 6) - 1;
    const B7: u8 = (1 << 7) - 1;
    const B8: u8 = u8::MAX;

    #[test]
    fn check_block0_max() {
        let block = u128::MAX;
        let data = Block0::decode(block);
        assert_eq!(data.partition, B4);
        assert_eq!(data.r, [B4; 6]);
        assert_eq!(data.g, [B4; 6]);
        assert_eq!(data.b, [B4; 6]);
        assert_eq!(data.p, [B1; 6]);
        assert_eq!(data.index_data, (1 << 45) - 1);
    }

    #[test]
    fn check_block1_max() {
        let block = u128::MAX;
        let data = Block1::decode(block);
        assert_eq!(data.partition, B6);
        assert_eq!(data.r, [B6; 4]);
        assert_eq!(data.g, [B6; 4]);
        assert_eq!(data.b, [B6; 4]);
        assert_eq!(data.p, [B1; 2]);
        assert_eq!(data.index_data, (1 << 46) - 1);
    }

    #[test]
    fn check_block2_max() {
        let block = u128::MAX;
        let data = Block2::decode(block);
        assert_eq!(data.partition, B6);
        assert_eq!(data.r, [B5; 6]);
        assert_eq!(data.g, [B5; 6]);
        assert_eq!(data.b, [B5; 6]);
        assert_eq!(data.index_data, (1 << 29) - 1);
    }

    #[test]
    fn check_block3_max() {
        let block = u128::MAX;
        let data = Block3::decode(block);
        assert_eq!(data.partition, B6);
        assert_eq!(data.r, [B7; 4]);
        assert_eq!(data.g, [B7; 4]);
        assert_eq!(data.b, [B7; 4]);
        assert_eq!(data.p, [B1; 4]);
        assert_eq!(data.index_data, (1 << 30) - 1);
    }

    #[test]
    fn check_block4_max() {
        let block = u128::MAX;
        let data = Block4::decode(block);
        assert_eq!(data.rot, Rotation::B);
        assert_eq!(data.idx_mode, true);
        assert_eq!(data.r, [B5; 2]);
        assert_eq!(data.g, [B5; 2]);
        assert_eq!(data.b, [B5; 2]);
        assert_eq!(data.a, [B6; 2]);
        assert_eq!(data.index_data0, (1 << 31) - 1);
        assert_eq!(data.index_data1, (1 << 47) - 1);
    }

    #[test]
    fn check_block5_max() {
        let block = u128::MAX;
        let data = Block5::decode(block);
        assert_eq!(data.rot, Rotation::B);
        assert_eq!(data.r, [B7; 2]);
        assert_eq!(data.g, [B7; 2]);
        assert_eq!(data.b, [B7; 2]);
        assert_eq!(data.a, [B8; 2]);
        assert_eq!(data.colors, (1 << 31) - 1);
        assert_eq!(data.alpha, (1 << 31) - 1);
    }

    #[test]
    fn check_block6_max() {
        let block = u128::MAX;
        let data = Block6::decode(block);
        assert_eq!(data.r, [B7; 2]);
        assert_eq!(data.g, [B7; 2]);
        assert_eq!(data.b, [B7; 2]);
        assert_eq!(data.a, [B7; 2]);
        assert_eq!(data.p, [B1; 2]);
        assert_eq!(data.index_data, (1 << 63) - 1);
    }

    #[test]
    fn check_block7_max() {
        let block = u128::MAX;
        let data = Block7::decode(block);
        assert_eq!(data.partition, B6);
        assert_eq!(data.r, [B5; 4]);
        assert_eq!(data.g, [B5; 4]);
        assert_eq!(data.b, [B5; 4]);
        assert_eq!(data.a, [B5; 4]);
        assert_eq!(data.p, [B1; 4]);
        assert_eq!(data.index_data, (1 << 30) - 1);
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
