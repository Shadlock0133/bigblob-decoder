use std::{
    mem::size_of,
    ops::{BitAnd, BitOrAssign, Shl, ShlAssign, Sub},
};

use image::{Rgba, RgbaImage};

use crate::align_up;

use super::{Block0, Block1, Block2, Block3, Block4, Block5, Block6, Block7};

pub fn encode_bc7(image: RgbaImage) -> Vec<u8> {
    let (width, height) = image.dimensions();
    let awidth = align_up::<4>(width);
    let aheight = align_up::<4>(height);
    let block_count = awidth * aheight / 16;
    let pos_iter = (0..aheight / 4)
        .flat_map(|y| (0..awidth / 4).map(move |x| (4 * x, 4 * y)));

    let mut res = Vec::with_capacity(block_count as usize * size_of::<u128>());
    for (x, y) in pos_iter {
        let mut pixels = [[Rgba([0; 4]); 4]; 4];
        for dy in 0..4 {
            for dx in 0..4 {
                if let Some(pixel) = image.get_pixel_checked(x + dx, y + dy) {
                    pixels[dy as usize][dx as usize] = *pixel;
                }
            }
        }
        let block = encode_bc7_block(pixels);
        res.extend_from_slice(&block.to_le_bytes());
    }
    res
}

pub fn encode_bc7_block(pixels: [[Rgba<u8>; 4]; 4]) -> u128 {
    if pixels == [[Rgba([0; 4]); 4]; 4] {
        return 0x00000000_aaaaaaac_00000000_00000020_u128;
    }
    let uses_transparency = pixels.iter().flatten().any(|x| x.0[3] != 255);
    if uses_transparency {
        Block6 {
            r: [0b1111111; 2],
            g: [0b0; 2],
            b: [0b1111111; 2],
            a: [0b0011111; 2],
            p: [0b1; 2],
            index_data: 0,
        }
        .encode()
    } else {
        Block6 {
            r: [0b1111111; 2],
            g: [0b0; 2],
            b: [0b1111111; 2],
            a: [0b1111111; 2],
            p: [0b1; 2],
            index_data: 0,
        }
        .encode()
    }
}

/// Pushes `BITS` amount of bits from `value` into `dest`.
///
/// So pushing `0b_xyz` into `0b_uvw` will result in `0b_uvwxyz`.
fn put_bits<
    T: From<u8>
        + From<R>
        + Shl<usize, Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + ShlAssign<usize>
        + BitOrAssign
        + Copy,
    R,
    const BITS: usize,
>(
    dest: &mut T,
    value: R,
) {
    assert!(0 < BITS);
    assert!(BITS <= (8 * size_of::<T>()));

    let mask = (T::from(1) << BITS) - T::from(1);
    *dest <<= BITS;
    *dest |= T::from(value) & mask;
}

/// Puts `array` of values (of bit-size `BITS`) in reverse order into `dest`.
///
/// So `[0b_xxx, 0b_yyy]` with `BITS = 3` will end up pushing `0b_yyyxxx`.
fn put_bits_array_rev<
    T: From<u8>
        + From<R>
        + Shl<usize, Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + ShlAssign<usize>
        + BitOrAssign
        + Copy,
    R,
    const BITS: usize,
    const N: usize,
>(
    dest: &mut T,
    array: [R; N],
) {
    assert!(0 < BITS);
    assert!(BITS <= (8 * size_of::<T>()));

    let mask = (T::from(1) << BITS) - T::from(1);
    for value in array.into_iter().rev() {
        *dest <<= BITS;
        *dest |= T::from(value) & mask;
    }
}

trait Encode {
    fn encode(self) -> u128;
}

impl Encode for Block0 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 45>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 1, 6>(&mut ret, self.p);
        put_bits_array_rev::<_, _, 4, 6>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 4, 6>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 4, 6>(&mut ret, self.r);
        put_bits::<_, _, 4>(&mut ret, self.partition);
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 0;
        ret
    }
}

impl Encode for Block1 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 46>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 1, 2>(&mut ret, self.p);
        put_bits_array_rev::<_, _, 6, 4>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 6, 4>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 6, 4>(&mut ret, self.r);
        put_bits::<_, _, 6>(&mut ret, self.partition);
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 1;
        ret
    }
}

impl Encode for Block2 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 29>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 5, 6>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 5, 6>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 5, 6>(&mut ret, self.r);
        put_bits::<_, _, 6>(&mut ret, self.partition);
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 2;
        ret
    }
}

impl Encode for Block3 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 30>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 1, 4>(&mut ret, self.p);
        put_bits_array_rev::<_, _, 7, 4>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 7, 4>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 7, 4>(&mut ret, self.r);
        put_bits::<_, _, 6>(&mut ret, self.partition);
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 3;
        ret
    }
}

impl Encode for Block4 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 47>(&mut ret, self.index_data1);
        put_bits::<_, _, 31>(&mut ret, self.index_data0);
        put_bits_array_rev::<_, _, 6, 2>(&mut ret, self.a);
        put_bits_array_rev::<_, _, 5, 2>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 5, 2>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 5, 2>(&mut ret, self.r);
        put_bits::<_, _, 1>(&mut ret, self.idx_mode);
        put_bits::<_, _, 2>(&mut ret, self.rot.to_u2());
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 4;
        ret
    }
}

impl Encode for Block5 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 31>(&mut ret, self.alpha_index_data);
        put_bits::<_, _, 31>(&mut ret, self.color_index_data);
        put_bits_array_rev::<_, _, 8, 2>(&mut ret, self.a);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.r);
        put_bits::<_, _, 2>(&mut ret, self.rot.to_u2());
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 5;
        ret
    }
}

impl Encode for Block6 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 63>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 1, 2>(&mut ret, self.p);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.a);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 7, 2>(&mut ret, self.r);
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 6;
        ret
    }
}

impl Encode for Block7 {
    fn encode(self) -> u128 {
        let mut ret = 0;
        put_bits::<_, _, 30>(&mut ret, self.index_data);
        put_bits_array_rev::<_, _, 1, 4>(&mut ret, self.p);
        put_bits_array_rev::<_, _, 5, 4>(&mut ret, self.a);
        put_bits_array_rev::<_, _, 5, 4>(&mut ret, self.b);
        put_bits_array_rev::<_, _, 5, 4>(&mut ret, self.g);
        put_bits_array_rev::<_, _, 5, 4>(&mut ret, self.r);
        put_bits::<_, _, 6>(&mut ret, self.partition);
        // mode
        put_bits::<_, _, 1>(&mut ret, 1u8);
        ret <<= 7;
        ret
    }
}
