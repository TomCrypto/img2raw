use byteorder::{WriteBytesExt, LE};
use exitfailure::ExitFailure;
use failure::{bail, Error};
use half::f16;
use image::{guess_format, hdr, load_from_memory, ImageFormat};
use img2raw::{ColorSpace, DataFormat, Header};
use rayon::prelude::*;
use std::fs::{read, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use structopt::StructOpt;
use zerocopy::AsBytes;

#[derive(Debug, StructOpt)]
#[structopt(about, author, name = "img2raw")]
struct Arguments {
    #[structopt(long, parse(try_from_str = parse_color_space))]
    source_color_space: ColorSpace,

    #[structopt(long, parse(try_from_str = parse_color_space))]
    output_color_space: ColorSpace,

    #[structopt(long = "format", parse(try_from_str = parse_data_format))]
    output_data_format: DataFormat,

    #[structopt(parse(from_os_str))]
    source_file: PathBuf,

    #[structopt(parse(from_os_str))]
    output_file: PathBuf,

    #[structopt(long)]
    header: bool,
}

fn parse_color_space(input: &str) -> Result<ColorSpace, Error> {
    if let Ok(color_space) = input.parse() {
        Ok(color_space)
    } else {
        bail!("unknown color space {}", input)
    }
}

fn parse_data_format(input: &str) -> Result<DataFormat, Error> {
    if let Ok(data_format) = input.parse() {
        Ok(data_format)
    } else {
        bail!("unknown data format {}", input)
    }
}

fn main() -> Result<(), ExitFailure> {
    Ok(run()?)
}

fn run() -> Result<(), Error> {
    let args = Arguments::from_args();

    let bytes = read(args.source_file)?;

    let mut image = match guess_format(&bytes)? {
        ImageFormat::HDR => load_hdr_image(&bytes)?,
        ImageFormat::PNG => load_dynamic_image(&bytes)?,
        ImageFormat::JPEG => load_dynamic_image(&bytes)?,
        ImageFormat::PNM => load_dynamic_image(&bytes)?,
        ImageFormat::BMP => load_dynamic_image(&bytes)?,
        ImageFormat::TIFF => load_dynamic_image(&bytes)?,
        unsupported => bail!("unsupported file type: {:?}", unsupported),
    };

    let source_color_space = args.source_color_space;
    let output_color_space = args.output_color_space;

    if source_color_space != output_color_space {
        if source_color_space == ColorSpace::NonColor {
            bail!("non-color source requires non-color output");
        }

        if output_color_space == ColorSpace::NonColor {
            bail!("non-color output requires non-color source");
        }

        image.pixels.par_iter_mut().for_each(|pixel| {
            *pixel = pixel.convert_into_cie_xyz(source_color_space);
            *pixel = pixel.convert_from_cie_xyz(output_color_space);
        });
    }

    let mut file = BufWriter::new(File::create(args.output_file)?);

    if args.header {
        let header = Header {
            color_space: args.output_color_space.into(),
            data_format: args.output_data_format.into(),
            dimensions: [image.width, image.height],
        };

        file.write_all(header.as_bytes())?;
    }

    match args.output_data_format {
        DataFormat::R32F => store_r32f_pixels(&image, file)?,
        DataFormat::RG32F => store_rg32f_pixels(&image, file)?,
        DataFormat::RGBA32F => store_rgba32f_pixels(&image, file)?,
        DataFormat::R8 => store_r8_pixels(&image, file)?,
        DataFormat::PackedR8 => store_packed_r8_pixels(&image, file)?,
        DataFormat::R16F => store_r16f_pixels(&image, file)?,
        DataFormat::RG16F => store_rg16f_pixels(&image, file)?,
        DataFormat::RGBA16F => store_rgba16f_pixels(&image, file)?,
        DataFormat::PackedR16F => store_packed_r16f_pixels(&image, file)?,
        DataFormat::RGBE8 => store_rgbe8_pixels(&image, file)?,
        DataFormat::RGBA8 => store_rgba8_pixels(&image, file)?,
    }

    println!(
        "{:?} {:?} {} {}",
        args.output_color_space, args.output_data_format, image.width, image.height
    );

    Ok(())
}

// Input

fn load_dynamic_image(bytes: &[u8]) -> Result<Image, Error> {
    let data = load_from_memory(bytes)?.to_rgba();

    let mut image = Image::new(data.width(), data.height());

    for (input, pixel) in data.pixels().zip(&mut image.pixels) {
        pixel.r = input.0[0] as f64 / 255.0;
        pixel.g = input.0[1] as f64 / 255.0;
        pixel.b = input.0[2] as f64 / 255.0;
        pixel.a = input.0[3] as f64 / 255.0;
    }

    Ok(image)
}

fn load_hdr_image(bytes: &[u8]) -> Result<Image, Error> {
    let loaded = hdr::HDRDecoder::new(bytes)?;

    let metadata = loaded.metadata();

    let data = loaded.read_image_hdr()?;

    let mut image = Image::new(metadata.width, metadata.height);

    for (input, pixel) in data.iter().zip(&mut image.pixels) {
        pixel.r = input.0[0] as f64;
        pixel.g = input.0[1] as f64;
        pixel.b = input.0[2] as f64;
    }

    Ok(image)
}

// Processing

#[derive(Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Pixel>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![Pixel::default(); (width * height) as usize],
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Pixel {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Pixel {
    pub fn convert_into_cie_xyz(self, color_space: ColorSpace) -> Self {
        match color_space {
            ColorSpace::NonColor | ColorSpace::CIEXYZ => self,
            ColorSpace::LinearSRGB => self.convert_into_cie_xyz_from_linear_srgb(),
            ColorSpace::SRGB => {
                let mut pixel = self;

                pixel.r = Self::convert_from_gamma_srgb(pixel.r);
                pixel.g = Self::convert_from_gamma_srgb(pixel.g);
                pixel.b = Self::convert_from_gamma_srgb(pixel.b);

                pixel.convert_into_cie_xyz_from_linear_srgb()
            }
        }
    }

    pub fn convert_from_cie_xyz(self, color_space: ColorSpace) -> Self {
        match color_space {
            ColorSpace::NonColor | ColorSpace::CIEXYZ => self,
            ColorSpace::LinearSRGB => self.convert_from_cie_xyz_into_linear_srgb(),
            ColorSpace::SRGB => {
                let mut pixel = self.convert_from_cie_xyz_into_linear_srgb();

                pixel.r = Self::convert_into_gamma_srgb(pixel.r);
                pixel.g = Self::convert_into_gamma_srgb(pixel.g);
                pixel.b = Self::convert_into_gamma_srgb(pixel.b);

                pixel
            }
        }
    }

    fn convert_into_cie_xyz_from_linear_srgb(self) -> Self {
        let mut pixel = self;

        pixel.r = 0.4124 * self.r + 0.3576 * self.g + 0.1805 * self.b;
        pixel.g = 0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b;
        pixel.b = 0.0193 * self.r + 0.1192 * self.g + 0.9505 * self.b;

        pixel
    }

    fn convert_from_cie_xyz_into_linear_srgb(self) -> Self {
        let mut pixel = self;

        pixel.r = 3.2406 * self.r - 1.5372 * self.g - 0.4986 * self.b;
        pixel.g = -0.9689 * self.r + 1.8758 * self.g + 0.0415 * self.b;
        pixel.b = 0.0557 * self.r - 0.2040 * self.g + 1.0570 * self.b;

        pixel
    }

    fn convert_into_gamma_srgb(x: f64) -> f64 {
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }

    fn convert_from_gamma_srgb(x: f64) -> f64 {
        if x <= 0.040_45 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    }
}

// Output

fn store_r32f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_f32::<LE>(pixel.r as f32)?;
    }

    Ok(())
}

fn store_rg32f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_f32::<LE>(pixel.r as f32)?;
        writer.write_f32::<LE>(pixel.g as f32)?;
    }

    Ok(())
}

fn store_rgba32f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_f32::<LE>(pixel.r as f32)?;
        writer.write_f32::<LE>(pixel.g as f32)?;
        writer.write_f32::<LE>(pixel.b as f32)?;
        writer.write_f32::<LE>(pixel.a as f32)?;
    }

    Ok(())
}

fn store_r8_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    let row_padding = (4 - image.width % 4) % 4;

    for y in 0..image.height {
        for x in 0..image.width {
            let pixel = image.pixels[(y * image.width + x) as usize];

            writer.write_u8((pixel.r.min(1.0).max(0.0) * 255.0) as u8)?;
        }

        for _ in 0..row_padding {
            writer.write_u8(0)?;
        }
    }

    Ok(())
}

fn store_packed_r8_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_u8((pixel.r.min(1.0).max(0.0) * 255.0) as u8)?;
    }

    Ok(())
}

fn safe_f64_to_f16(x: f64) -> f16 {
    f16::from_f64(x.max(-65504.0).min(65504.0))
}

fn store_r16f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    let row_padding = image.width % 2;

    for y in 0..image.height {
        for x in 0..image.width {
            let pixel = image.pixels[(y * image.width + x) as usize];

            writer.write_u16::<LE>(safe_f64_to_f16(pixel.r).to_bits())?;
        }

        for _ in 0..row_padding {
            writer.write_u16::<LE>(0)?;
        }
    }

    Ok(())
}

fn store_rg16f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.r).to_bits())?;
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.g).to_bits())?;
    }

    Ok(())
}

fn store_rgba16f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.r).to_bits())?;
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.g).to_bits())?;
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.b).to_bits())?;
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.a).to_bits())?;
    }

    Ok(())
}

fn store_packed_r16f_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_u16::<LE>(safe_f64_to_f16(pixel.r).to_bits())?;
    }

    Ok(())
}

fn store_rgbe8_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        let v = pixel.r.max(pixel.g).max(pixel.b);

        if v < 1e-32 {
            writer.write_u8(0)?;
            writer.write_u8(0)?;
            writer.write_u8(0)?;
            writer.write_u8(0)?;
        } else {
            let (f, e) = frexp(v);

            let r_byte = (pixel.r * f * 256.0 / v).min(255.0).max(0.0) as u8;
            let g_byte = (pixel.g * f * 256.0 / v).min(255.0).max(0.0) as u8;
            let b_byte = (pixel.b * f * 256.0 / v).min(255.0).max(0.0) as u8;

            writer.write_u8(r_byte)?;
            writer.write_u8(g_byte)?;
            writer.write_u8(b_byte)?;
            writer.write_u8((e + 128) as u8)?;
        }
    }

    Ok(())
}

// https://stackoverflow.com/a/55696477/10471467
fn frexp(s: f64) -> (f64, i32) {
    if 0.0 == s {
        (s, 0)
    } else {
        let lg = s.abs().log2();
        let x = (lg - lg.floor() - 1.0).exp2();
        let exp = lg.floor() + 1.0;
        (s.signum() * x, exp as i32)
    }
}

fn store_rgba8_pixels<W: Write>(image: &Image, mut writer: W) -> Result<(), Error> {
    for pixel in &image.pixels {
        writer.write_u8((pixel.r.min(1.0).max(0.0) * 255.0) as u8)?;
        writer.write_u8((pixel.g.min(1.0).max(0.0) * 255.0) as u8)?;
        writer.write_u8((pixel.b.min(1.0).max(0.0) * 255.0) as u8)?;
        writer.write_u8((pixel.a.min(1.0).max(0.0) * 255.0) as u8)?;
    }

    Ok(())
}
