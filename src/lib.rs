//! This crate provides type definitions to parse `img2raw` headers.

#![forbid(missing_docs)]
#![forbid(unsafe_code)]
#![no_std]

use core::fmt::{Display, Formatter, Result as FmtResult};
use core::str::FromStr;
use zerocopy::{AsBytes, FromBytes};

/// Header optionally prepended to the pixel data.
#[repr(C)]
#[derive(AsBytes, FromBytes, Clone, Copy, Debug)]
pub struct Header {
    /// The color space of the subsequent pixel data.
    pub color_space: ColorSpaceInfo,
    /// The data format of the subsequent pixel data.
    pub data_format: DataFormatInfo,
    /// The image width and height in pixels.
    pub dimensions: [u32; 2],
}

/// Color space information stored in a header.
///
/// The header might not be valid, so this is an intermediate struct which is
/// used to catch invalid bit patterns not representable by any enum variant.
#[repr(transparent)]
#[derive(AsBytes, Clone, Copy, Debug, Eq, FromBytes, Hash, PartialEq)]
pub struct ColorSpaceInfo(u32);

impl ColorSpaceInfo {
    /// Returns the inner color space if it is valid.
    pub fn try_parse(self) -> Option<ColorSpace> {
        ColorSpace::try_from_u32(self.0)
    }
}

impl From<ColorSpace> for ColorSpaceInfo {
    fn from(color_space: ColorSpace) -> Self {
        Self(color_space as u32)
    }
}

/// Data format information stored in a header.
///
/// The header might not be valid, so this is an intermediate struct which is
/// used to catch invalid bit patterns not representable by any enum variant.
#[repr(transparent)]
#[derive(AsBytes, Clone, Copy, Debug, Eq, FromBytes, Hash, PartialEq)]
pub struct DataFormatInfo(u32);

impl DataFormatInfo {
    /// Returns the inner data format if it is valid.
    pub fn try_parse(self) -> Option<DataFormat> {
        DataFormat::try_from_u32(self.0)
    }
}

impl From<DataFormat> for DataFormatInfo {
    fn from(data_format: DataFormat) -> Self {
        Self(data_format as u32)
    }
}

/// Parsing error for a color space or data format.
pub struct UnknownVariant {}

macro_rules! gen_enum {
    ($name:ident, $doc:expr => [$([$variant:ident = $value:expr, $variant_doc:expr],)+]) => {
        #[repr(u32)]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[doc = $doc] pub enum $name { $(#[doc = $variant_doc] $variant = $value,)+ }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter) -> FmtResult {
                match self { $(Self::$variant => write!(f, stringify!($variant)),)+ }
            }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $(stringify!($variant) => Ok(Self::$variant),)+
                    _ => Err(UnknownVariant {})
                }
            }
        }

        impl $name {
            pub(crate) fn try_from_u32(value: u32) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    }
}

// NOTE: the variant discriminant values are specified explicitly here so that
// they aren't accidentally changed during reordering, which would break code.

gen_enum!(ColorSpace, "Available color spaces for the pixel data." => [
    [NonColor = 0, "The pixel data does not contain color information."],
    [CIEXYZ = 1, "The CIE XYZ 1931 color space using the D65 illuminant."],
    [SRGB = 2, "The sRGB color space as defined by IEC 61966-2-1:1999."],
    [LinearSRGB = 3, "The sRGB color space but without gamma correction, i.e. linear."],
]);

gen_enum!(DataFormat, "Available data formats for the pixel data." => [
    [R32F = 0, "32-bit floating-point, 4-byte row alignment."],
    [RG32F = 1, "32-bit floating-point, 4-byte row alignment."],
    [RGBA32F = 2, "32-bit floating-point, 4-byte row alignment."],
    [R8 = 3, "8-bit fixed-point, 4-byte row alignment."],
    [PackedR8 = 4, "8-bit fixed-point, 1-byte row alignment."],
    [R16F = 5, "16-bit floating-point, 4-byte row alignment."],
    [RG16F = 6, "16-bit floating-point, 4-byte row alignment."],
    [RGBA16F = 7, "16-bit floating-point, 4-byte row alignment."],
    [PackedR16F = 8, "16-bit floating-point, 2-byte row alignment."],
    [RGBE8 = 9, "8-bit RGBE, alpha is exponent, 4-byte row alignment."],
    [RGBA8 = 10, "8-bit fixed-point, 4-byte row alignment."],
]);
