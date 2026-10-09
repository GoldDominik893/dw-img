# DWXG binary format

All integer fields are **unsigned 32-bit big-endian**. Pixels are stored from top row to bottom row, left to right within each row. There is no per-row padding in 8-bit or 16-bit modes.

## Current format (24-byte header)

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 bytes | ASCII signature `DWXG` |
| 4 | 4 bytes | Image width in pixels |
| 8 | 4 bytes | Image height in pixels |
| 12 | 4 bytes | Compression flag: `0` = uncompressed, `1` = zlib/DEFLATE compressed |
| 16 | 4 bytes | Bits per pixel: `16` = RGB565, `8` = grayscale |
| 20 | 4 bytes | Stored payload length in bytes |
| 24 | N bytes | Pixel payload |

`24 + payload_length` must equal the file size. Width and height must be positive. Implementations should reject invalid dimensions and unreasonable pixel counts before allocating image memory.

## Pixel payload

### 16-bit RGB565

Each pixel is a 16-bit word in **big-endian byte order**:

- bits 15–11: red, 5 bits
- bits 10–5: green, 6 bits
- bits 4–0: blue, 5 bits

For pixel channels `(R, G, B)` in the 0–255 range, non-dithered encoding uses:

```text
word = ((R >> 3) << 11) | ((G >> 2) << 5) | (B >> 3)
```

To display the value, expand the channels to 8 bits. One common expansion is `(r5 << 3) | (r5 >> 2)`, `(g6 << 2) | (g6 >> 4)`, `(b5 << 3) | (b5 >> 2)`.

### 8-bit grayscale

One byte per pixel, with luminance calculated as approximately:

```text
gray = round(0.299 * R + 0.587 * G + 0.114 * B)
```

This is grayscale, not indexed/palette colour.

## Compression

When the compression flag is `1`, only the pixel payload is compressed using a zlib-wrapped DEFLATE stream. The payload-length field stores the **compressed** byte length. When the flag is `0`, it stores the raw pixel byte length.

Expected uncompressed payload sizes:

- 16-bit RGB565: `width * height * 2`
- 8-bit grayscale: `width * height`

## Recognised earlier layouts

Readers in this project also attempt to read the identifiable layouts from the earlier Python iterations:

1. **20-byte RGB565 header:** `DWXG`, width, height, compression flag, payload length, payload. This was the older compression-capable version and always uses RGB565.
2. **16-byte raw header:** `DWXG`, width, height, reserved value `0`, raw bytes. Recognised as RGB565 when payload size is exactly `width * height * 2`; otherwise recognised as 1-bit monochrome when payload size is `ceil(width / 8) * height`, with each row byte-padded and the leftmost pixel stored in the most significant bit.

The 1-bit legacy layout is only recognised when its payload length unambiguously matches the expected row-padded size. New files should use the 24-byte format.
