# img2raw

This crate provides a simple command-line tool `img2raw` which takes any supported raster image format (e.g. PNG, JPEG, HDR, ...) and outputs the raw pixel contents in scanline order according to some data format such as RGBA8 or R16F suitable for use in rendering applications. It supports basic color space conversions, but does not detect the source color space automatically.

By default the tool will output the raw pixel data and nothing else, so additional metadata needs to be associated with the output file for use in further applications. However, the tool also supports writing out a simple 16-byte header at the start of the output containing the data width, height, data format and color space. This header can be parsed using the type definitions in this crate. The pixel data immediately follows this header if present.

    cargo install img2raw

## License

This software is provided under the MIT license.

## Rationale

The existence of this tool is motivated by me often needing to convert external assets into a format suitable for fast and efficient loading into graphics APIs in my own renderers and not wanting to google arcane imagemagick commands or constantly having to write one-off image converters. It's meant to be a simple, stable and easy-to-use program to process basic two-dimensional RGBA data into a number of known formats.

## Supported Formats

Below is a table of all currently supported formats, though adding more is easy. Most formats have a 4-byte row alignment for compatibility with common graphics APIs, but some (when applicable) have a "packed" variant where padding bytes are never inserted at the end of each row. The "RGBA" notation refers only to the abstract channels the pixel data is contained in; the data may not be in an RGB color space, and may not even be a color, depending on the intended usage and target application.

| Data format     | Channels | Component data type      | Range     | Row alignment | Row padding  | Notes                                                   |
| :-------------- | :------- | :----------------------: | :-------: | :------------ | :----------- | :------------------------------------------------------ |
| `R32F`          | `R`      | 32-bit floating-point    | (-∞, +∞)  | 4-byte        | Never        |                                                         |
| `RG32F`         | `RG`     | 32-bit floating-point    | (-∞, +∞)  | 4-byte        | Never        |                                                         |
| `RGBA32F`       | `RGBA`   | 32-bit floating-point    | (-∞, +∞)  | 4-byte        | Never        |                                                         |
| `R16F`          | `R`      | 16-bit floating-point    | (-∞, +∞)  | 4-byte        | 0 or 2 bytes |                                                         |
| `PackedR16F`    | `R`      | 16-bit floating-point    | (-∞, +∞)  | 2-byte        | Never        | Packed variant of `R16F`.                               |
| `RG16F`         | `RG`     | 16-bit floating-point    | (-∞, +∞)  | 4-byte        | Never        |                                                         |
| `RGBA16F`       | `RGBA`   | 16-bit floating-point    | (-∞, +∞)  | 4-byte        | Never        |                                                         |
| `R8`            | `R`      | 8-bit fixed-point        | [0, 1]    | 4-byte        | 0 to 3 bytes |                                                         |
| `PackedR8`      | `R`      | 8-bit fixed-point        | [0, 1]    | 1-byte        | Never        | Packed variant of `R8`.                                 |
| `RGBE8`         | `RGBA`   | 8-bit special encoding   | (0, +∞)   | 4-byte        | Never        | RGBE encoding, alpha channel contains exponent.         |

Currently, the source pixel data is silently clamped to the output format's range, and no attention is paid to floating-point infinities or NaNs. Warnings may be logged in a future version.

## Supported Color Spaces

The color space support is very minimalistic, really the bare minimum to be able to know what kind of color data is actually being written out. It supports gamma-corrected sRGB, linear sRGB and CIE XYZ colors. Non-color data is also "supported" by simply not doing any processing on the image pixel data and simply writing it out as-is. Use the `NonColor` "color space" for **both** source and output to use the non-color path, using it on only one is a logic error.

| Color space       | Description                                                                           |
| :---------------- | :------------------------------------------------------------------------------------ |
| `NonColor`        | The pixel data does not contain color information.                                    |
| `CIEXYZ`          | The CIE XYZ 1931 color space using the D65 illuminant.                                |
| `SRGB`            | The sRGB color space as defined by IEC 61966-2-1:1999.                                |
| `LinearSRGB`      | The sRGB color space but without gamma correction, i.e. linear.                       |
