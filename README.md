# DW eXtra Good Image Format (DWXG)

DWXG is a cool image format based in python which is revolutionary with its features and easy of use, and soon will be integrated within websites and other software.

## Features

- DWXG image format.
- Usable on websites, via the javascript library
- A rust application and a python application.
- Convert .jpg .jpeg, .png .bmp files to .dwxg.
- View DWXG image files.
- Compression(lossless) and Dithering support.

## Implementation

Currently, DWXG has 3 ways of using the format

- Using the javascript library
- Using the compiled rust program
- Using the python program

Below you will find a guide on each one.

### JS Library

#### Using the built in DWXG image viewer

The library comes with a built in image viewer like the rust and python programs do which allows you to select files from your computer and visualise them on the website directly.

```html
<div id="viewer" style="height: 620px"></div>
<script type="module">
  import { mountDwxgViewer } from './dwxg-viewer.js';
  const viewer = mountDwxgViewer(document.querySelector('#viewer'));
</script>
```

`mountDwxgViewer()` uses the designated element in the document as the viewer.

#### Use a DWXG file as an ordinary `<img>`

Browsers cannot decode a custom `.dwxg` URL by themselves. The helper `dwxgSrc(url)` fetches and decodes the file, converts it to a PNG data URL, and returns a value the browser can display in a normal image element.

```html
<img id="photo" alt="A DWXG image">
<script type="module">
  import { dwxgSrc, setDwxgSrc, loadDwxgImages } from './dwxg-viewer.js';

  // Option A: get a browser-readable src string
  document.querySelector('#photo').src = await dwxgSrc('/images/photo.dwxg');

  // Option B: helper assigns src for you
  await setDwxgSrc(document.querySelector('#photo'), '/images/photo.dwxg');
</script>
```

Or mark multiple images in HTML and initialize them in one call:

```html
<img data-dwxg-src="/images/one.dwxg" alt="First image">
<img data-dwxg-src="/images/two.dwxg" alt="Second image">
<script type="module">
  import { loadDwxgImages } from './dwxg-viewer.js';
  const results = await loadDwxgImages();
  // Each result has ok:true/false; failed elements also receive data-dwxg-error.
</script>
```

`dwxgSrc()` returns a PNG data URL, not the original DWXG URL. This means it can be used in `<img src>`, CSS backgrounds, and other browser image APIs. Remote files require normal `fetch()` access and CORS permission from the image host.

### Rust program

#### Ready to use rust program

This repository contains a pre built rust program, it is available in the releases tab.

#### Complilation of the rust program

From the rust directory:

```bash
cargo test
cargo run
cargo build --release
```

The app's source is in `src/main.rs` and the shared DWXG codec is in `src/format.rs`.

Then launch the executable at `target/release`

### Python program

This repository contains a ready to use python file at `/python/main.py`.

## Screenshots

### Javascript version

### Rust version

### Python Version

Main window

![image](https://github.com/user-attachments/assets/75ed3ed0-e168-44e5-85dd-e0cdf62a3e08)

Conversion window (to DWXG)

![image](https://github.com/user-attachments/assets/462f18a1-de3d-4628-864d-17e76d995ca7)

Viewing coco.dwxg

![image](https://github.com/user-attachments/assets/a7b9d4d7-0b09-4a74-ab1c-4d6a1389789a)
