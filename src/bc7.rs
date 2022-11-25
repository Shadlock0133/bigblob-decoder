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
        let pixels = decode_bc7_block(block).unwrap();
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
struct Block5 {
    #[deku(bits = 6)]
    _mode: u8,
    rot: RotA,
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
#[deku(endian = "little", type = "u8", bits = 2, ctx = "_: deku::ctx::Endian")]
enum RotA {
    No = 0,
    R = 1,
    G = 2,
    B = 3,
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
            Index1::I0 => Index2::I0,
            Index1::I1 => Index2::I1,
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

const WEIGHT2: [u16; 4] = [0, 21, 43, 64];
const WEIGHT3: [u16; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHT4: [u16; 16] =
    [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
const WEIGHTS: [&[u16]; 3] = [&WEIGHT2, &WEIGHT3, &WEIGHT4];
fn interpolate<const BITS: usize>(a: u8, b: u8, index: usize) -> u8 {
    let a = (64 - WEIGHTS[BITS - 2][index]) * a as u16;
    let b = WEIGHTS[BITS - 2][index] * b as u16;
    ((a + b + 32) >> 6) as u8
}

fn decode_bc7_block(block: u128) -> Result<[[Rgba<u8>; 4]; 4], ()> {
    // TODO: implement it
    // if block == 0xaaaaaaac_00000000_00000020 {
    //     return Ok([[Rgba([0; 4]); 4]; 4]);
    // }
    let mode = block.trailing_zeros();
    match mode {
        0 => return Ok([[Rgba([0, 0, 0, 255]); 4]; 4]),
        1 => return Ok([[Rgba([0, 0, 255, 255]); 4]; 4]),
        2 => return Ok([[Rgba([0, 255, 0, 255]); 4]; 4]),
        3 => return Ok([[Rgba([0, 255, 255, 255]); 4]; 4]),
        4 => return Ok([[Rgba([255, 0, 0, 255]); 4]; 4]),
        5 => {
            // FIXME: output doesn't seem correct
            let le_bytes = block.to_le_bytes();
            let data = Block5::from_bytes((&le_bytes, 0)).unwrap().1;

            let color0 = Rgb([data.r0 << 1, data.g0 << 1, data.b0 << 1]);
            let color3 = Rgb([data.r1 << 1, data.g1 << 1, data.b1 << 1]);
            let color1 = color0.map2(&color3, |a, b| interpolate::<2>(a, b, 1));
            let color2 = color0.map2(&color3, |a, b| interpolate::<2>(a, b, 2));

            let alpha0 = data.a0;
            let alpha3 = data.a1;
            let alpha1 = interpolate::<2>(alpha0, alpha3, 1);
            let alpha2 = interpolate::<2>(alpha0, alpha3, 2);

            let mut ret = [[Rgba([0; 4]); 4]; 4];
            let colors =
                data.colors.0.into_iter().chain([data.colors.1.into()]);
            let alphas = data.alpha.0.into_iter().chain([data.alpha.1.into()]);
            for ((rgba, ci), ai) in
                ret.iter_mut().flatten().zip(colors).zip(alphas)
            {
                let color = match ci {
                    Index2::I0 => color0,
                    Index2::I1 => color1,
                    Index2::I2 => color2,
                    Index2::I3 => color3,
                };
                let alpha = match ai {
                    Index2::I0 => alpha0,
                    Index2::I1 => alpha1,
                    Index2::I2 => alpha2,
                    Index2::I3 => alpha3,
                };
                let [rgb @ .., a] = &mut rgba.0;
                *rgb = color.0;
                *a = alpha;
                match data.rot {
                    RotA::No => (),
                    RotA::R => std::mem::swap(&mut rgb[0], a),
                    RotA::G => std::mem::swap(&mut rgb[1], a),
                    RotA::B => std::mem::swap(&mut rgb[2], a),
                }
            }
            return Ok(ret);
        }
        6 => return Ok([[Rgba([255, 255, 0, 255]); 4]; 4]),
        7 => return Ok([[Rgba([255, 255, 255, 255]); 4]; 4]),
        8.. => return Err(()),
    }
}

#[cfg(test)]
mod tests {
    use deku::DekuContainerRead;

    use crate::bc7::Block5;

    #[test]
    fn test() {
        let block = 0xaaaaaaac_00000000_00000020_u128;
        let le_bytes = block.to_le_bytes();
        let (rest, _data) = Block5::from_bytes((&le_bytes, 0)).unwrap();
        assert_eq!(rest, (&[0u8; 0][..], 0));
        // eprintln!("block5: {block:#x}: {data:#?}");
    }
}
