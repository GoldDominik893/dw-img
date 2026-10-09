/**
 * DWXG browser library.
 * Current DWXG files use a 24-byte big-endian header and RGB565 / grayscale pixels.
 * Browsers with DecompressionStream('deflate') support zlib-compressed files.
 */

export const DWXG_HEADER_SIZE = 24;
export const DWXG_MAX_PIXELS = 100_000_000;

function bytesOf(input) {
  if (input instanceof Uint8Array) return input;
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  throw new TypeError('Expected a Uint8Array, ArrayBuffer, or typed-array view.');
}

function u32(view, offset) {
  if (offset + 4 > view.byteLength) throw new Error('DWXG header is truncated.');
  return view.getUint32(offset, false);
}

function validateDimensions(width, height, maxPixels = DWXG_MAX_PIXELS) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1) {
    throw new Error('DWXG width and height must be positive integers.');
  }
  const pixels = width * height;
  if (!Number.isSafeInteger(pixels) || pixels > maxPixels) {
    throw new Error(`Image has too many pixels (${pixels}; limit ${maxPixels}).`);
  }
  return pixels;
}

async function inflateZlib(payload, maximumOutputBytes) {
  if (typeof DecompressionStream === 'undefined') {
    throw new Error("This browser does not support DecompressionStream('deflate'); use a recent browser or an uncompressed DWXG file.");
  }
  try {
    const stream = new Blob([payload]).stream().pipeThrough(new DecompressionStream('deflate'));
    const reader = stream.getReader();
    const chunks = [];
    let total = 0;
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > maximumOutputBytes) {
        await reader.cancel();
        throw new Error('Decompressed payload exceeds the expected image size.');
      }
      chunks.push(value);
    }
    const output = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) { output.set(chunk, offset); offset += chunk.byteLength; }
    return output;
  } catch (error) {
    throw new Error(`Could not decompress DWXG pixel data: ${error.message}`);
  }
}

async function deflateZlib(payload) {
  if (typeof CompressionStream === 'undefined') {
    throw new Error("This browser does not support CompressionStream('deflate'). Set compress:false or use a recent browser.");
  }
  const stream = new Blob([payload]).stream().pipeThrough(new CompressionStream('deflate'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

function rgb565ToRgb(payload, pixelCount) {
  if (payload.byteLength !== pixelCount * 2) {
    throw new Error('RGB565 data length does not match image dimensions.');
  }
  const rgb = new Uint8ClampedArray(pixelCount * 3);
  for (let p = 0, i = 0; p < pixelCount; p++, i += 2) {
    const word = (payload[i] << 8) | payload[i + 1];
    const r5 = (word >>> 11) & 0x1f;
    const g6 = (word >>> 5) & 0x3f;
    const b5 = word & 0x1f;
    const out = p * 3;
    rgb[out] = (r5 << 3) | (r5 >>> 2);
    rgb[out + 1] = (g6 << 2) | (g6 >>> 4);
    rgb[out + 2] = (b5 << 3) | (b5 >>> 2);
  }
  return rgb;
}

function grayscaleToRgb(payload, pixelCount) {
  if (payload.byteLength !== pixelCount) throw new Error('Grayscale data length does not match image dimensions.');
  const rgb = new Uint8ClampedArray(pixelCount * 3);
  for (let p = 0; p < pixelCount; p++) {
    const gray = payload[p];
    const i = p * 3;
    rgb[i] = gray;
    rgb[i + 1] = gray;
    rgb[i + 2] = gray;
  }
  return rgb;
}

function monoToRgb(payload, width, height, pixelCount) {
  const rowBytes = Math.ceil(width / 8);
  if (payload.byteLength !== rowBytes * height) throw new Error('1-bit data length does not match image dimensions.');
  const rgb = new Uint8ClampedArray(pixelCount * 3);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const set = (payload[y * rowBytes + Math.floor(x / 8)] & (1 << (7 - (x % 8)))) !== 0;
      const value = set ? 255 : 0;
      const i = (y * width + x) * 3;
      rgb[i] = rgb[i + 1] = rgb[i + 2] = value;
    }
  }
  return rgb;
}

function makeResult(width, height, rgb, bitsPerPixel, compressed, legacy, fileBytes) {
  return { width, height, rgb, bitsPerPixel, compressed, legacy, fileBytes };
}

/** Parse a DWXG file into RGB bytes and metadata. */
export async function decodeDwxg(input, options = {}) {
  const bytes = bytesOf(input);
  if (bytes.byteLength < 16 || bytes[0] !== 0x44 || bytes[1] !== 0x57 || bytes[2] !== 0x58 || bytes[3] !== 0x47) {
    throw new Error('This file does not have a valid DWXG signature.');
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = u32(view, 4);
  const height = u32(view, 8);
  const pixelCount = validateDimensions(width, height, options.maxPixels ?? DWXG_MAX_PIXELS);

  // Current 24-byte header: flags at 12, bpp at 16, payload length at 20.
  if (bytes.byteLength >= DWXG_HEADER_SIZE) {
    const flag = u32(view, 12);
    const depth = u32(view, 16);
    const length = u32(view, 20);
    if (flag <= 1 && (depth === 8 || depth === 16) && length === bytes.byteLength - DWXG_HEADER_SIZE) {
      const storedPayload = bytes.subarray(DWXG_HEADER_SIZE);
      const payload = flag ? await inflateZlib(storedPayload, pixelCount * (depth === 16 ? 2 : 1)) : storedPayload;
      const rgb = depth === 16 ? rgb565ToRgb(payload, pixelCount) : grayscaleToRgb(payload, pixelCount);
      return makeResult(width, height, rgb, depth, Boolean(flag), false, bytes.byteLength);
    }
  }

  // Legacy 20-byte header: flags at 12, payload length at 16, always RGB565.
  if (bytes.byteLength >= 20) {
    const flag = u32(view, 12);
    const length = u32(view, 16);
    if (flag <= 1 && length === bytes.byteLength - 20) {
      const storedPayload = bytes.subarray(20);
      const payload = flag ? await inflateZlib(storedPayload, pixelCount * 2) : storedPayload;
      const rgb = rgb565ToRgb(payload, pixelCount);
      return makeResult(width, height, rgb, 16, Boolean(flag), true, bytes.byteLength);
    }
  }

  // Legacy 16-byte raw header: reserved zero followed by raw RGB565 or row-padded 1-bit pixels.
  if (u32(view, 12) === 0) {
    const payload = bytes.subarray(16);
    if (payload.byteLength === pixelCount * 2) {
      return makeResult(width, height, rgb565ToRgb(payload, pixelCount), 16, false, true, bytes.byteLength);
    }
    if (payload.byteLength === Math.ceil(width / 8) * height) {
      return makeResult(width, height, monoToRgb(payload, width, height, pixelCount), 1, false, true, bytes.byteLength);
    }
  }

  throw new Error('Unrecognised DWXG layout or corrupt pixel data.');
}

function packRgb565(rgb, width, height, dither) {
  const count = width * height;
  const out = new Uint8Array(count * 2);
  if (!dither) {
    for (let p = 0; p < count; p++) {
      const i = p * 3;
      const word = ((rgb[i] >>> 3) << 11) | ((rgb[i + 1] >>> 2) << 5) | (rgb[i + 2] >>> 3);
      out[p * 2] = word >>> 8;
      out[p * 2 + 1] = word & 0xff;
    }
    return out;
  }

  const current = Array.from({ length: width + 2 }, () => [0, 0, 0]);
  const next = Array.from({ length: width + 2 }, () => [0, 0, 0]);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 3;
      const adjusted = [0, 0, 0];
      for (let c = 0; c < 3; c++) adjusted[c] = Math.min(255, Math.max(0, rgb[i + c] + current[x + 1][c]));
      const qr = Math.round(adjusted[0] * 31 / 255);
      const qg = Math.round(adjusted[1] * 63 / 255);
      const qb = Math.round(adjusted[2] * 31 / 255);
      const word = (qr << 11) | (qg << 5) | qb;
      out[(y * width + x) * 2] = word >>> 8;
      out[(y * width + x) * 2 + 1] = word & 0xff;
      const quantized = [qr * 255 / 31, qg * 255 / 63, qb * 255 / 31];
      for (let c = 0; c < 3; c++) {
        const error = adjusted[c] - quantized[c];
        current[x + 2][c] += error * 7 / 16;
        next[x][c] += error * 3 / 16;
        next[x + 1][c] += error * 5 / 16;
        next[x + 2][c] += error / 16;
      }
    }
    for (let x = 0; x < width + 2; x++) {
      current[x][0] = current[x][1] = current[x][2] = 0;
    }
    for (let x = 0; x < width + 2; x++) {
      current[x] = next[x];
      next[x] = [0, 0, 0];
    }
  }
  return out;
}

/** Create a current-format DWXG file from {width, height, rgb: Uint8Array|Uint8ClampedArray}. */
export async function encodeDwxg(image, options = {}) {
  const width = Number(image?.width);
  const height = Number(image?.height);
  const count = validateDimensions(width, height, options.maxPixels ?? DWXG_MAX_PIXELS);
  const rgb = bytesOf(image.rgb);
  if (rgb.byteLength !== count * 3) throw new Error('The rgb buffer must contain exactly 3 bytes per pixel.');
  const depth = options.bitsPerPixel ?? 16;
  if (depth !== 8 && depth !== 16) throw new Error('bitsPerPixel must be 8 or 16.');
  let payload;
  if (depth === 16) {
    payload = packRgb565(rgb, width, height, Boolean(options.dither));
  } else {
    payload = new Uint8Array(count);
    for (let p = 0; p < count; p++) {
      const i = p * 3;
      payload[p] = Math.round((299 * rgb[i] + 587 * rgb[i + 1] + 114 * rgb[i + 2]) / 1000);
    }
  }
  const compressed = Boolean(options.compress);
  if (compressed) payload = await deflateZlib(payload);
  if (payload.byteLength > 0xffffffff) throw new Error('Encoded payload is too large for DWXG.');
  const output = new Uint8Array(DWXG_HEADER_SIZE + payload.byteLength);
  output.set([0x44, 0x57, 0x58, 0x47], 0);
  const header = new DataView(output.buffer);
  header.setUint32(4, width, false);
  header.setUint32(8, height, false);
  header.setUint32(12, compressed ? 1 : 0, false);
  header.setUint32(16, depth, false);
  header.setUint32(20, payload.byteLength, false);
  output.set(payload, DWXG_HEADER_SIZE);
  return output;
}


/**
 * Fetch a DWXG URL and convert it to a browser-native PNG data URL.
 * Use the returned string directly as an HTMLImageElement.src. The remote
 * server must permit cross-origin requests when the page is hosted elsewhere.
 */
export async function dwxgSrc(url, options = {}) {
  const response = await fetch(url, { signal: options.signal, credentials: options.credentials ?? 'same-origin' });
  if (!response.ok) throw new Error(`Could not fetch DWXG image (${response.status} ${response.statusText}).`);
  const decoded = await decodeDwxg(await response.arrayBuffer(), options);
  const canvas = typeof OffscreenCanvas !== 'undefined'
    ? new OffscreenCanvas(decoded.width, decoded.height)
    : Object.assign(document.createElement('canvas'), { width: decoded.width, height: decoded.height });
  canvas.width = decoded.width;
  canvas.height = decoded.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Could not create a 2D canvas context to render DWXG.');
  const rgba = new Uint8ClampedArray(decoded.width * decoded.height * 4);
  for (let src = 0, dst = 0; src < decoded.rgb.length; src += 3, dst += 4) {
    rgba[dst] = decoded.rgb[src];
    rgba[dst + 1] = decoded.rgb[src + 1];
    rgba[dst + 2] = decoded.rgb[src + 2];
    rgba[dst + 3] = 255;
  }
  context.putImageData(new ImageData(rgba, decoded.width, decoded.height), 0, 0);
  if (typeof canvas.convertToBlob === 'function') {
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result);
      reader.onerror = () => reject(new Error('Could not convert DWXG image to a PNG data URL.'));
      reader.readAsDataURL(blob);
    });
  }
  return canvas.toDataURL('image/png');
}

/** Fetch and decode a DWXG URL, then assign the resulting PNG data URL to an img element. */
export async function setDwxgSrc(img, url, options = {}) {
  if (!(img instanceof HTMLImageElement)) throw new TypeError('setDwxgSrc expects an HTMLImageElement.');
  img.src = await dwxgSrc(url, options);
  return img;
}

/** Load all elements marked with data-dwxg-src and set their native img src values. */
export async function loadDwxgImages(root = document, options = {}) {
  const images = [...root.querySelectorAll('img[data-dwxg-src]')];
  return Promise.all(images.map(async img => {
    const url = img.dataset.dwxgSrc;
    try {
      await setDwxgSrc(img, url, options);
      img.removeAttribute('data-dwxg-error');
      return { img, url, ok: true };
    } catch (error) {
      img.dataset.dwxgError = error.message || String(error);
      img.dispatchEvent(new CustomEvent('dwxgerror', { detail: { url, error } }));
      return { img, url, ok: false, error };
    }
  }));
}

const CSS = `
.dwxg-viewer{display:flex;flex-direction:column;min-width:0;min-height:260px;height:100%;font:14px/1.4 system-ui,sans-serif;color:var(--dwxg-fg,#202124);background:var(--dwxg-bg,#fff);border:1px solid #d7d9dd;border-radius:8px;overflow:hidden}
.dwxg-toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:8px;border-bottom:1px solid #e2e4e8;background:var(--dwxg-toolbar,#f5f6f8)}
.dwxg-toolbar button,.dwxg-toolbar label{border:1px solid #c8ccd2;border-radius:5px;padding:5px 9px;background:#fff;color:#202124;cursor:pointer;font:inherit}
.dwxg-toolbar button:hover,.dwxg-toolbar label:hover{background:#edf2f7}
.dwxg-toolbar input[type=range]{width:115px}
.dwxg-meta{font-size:12px;color:#4e5661;white-space:normal;overflow-wrap:anywhere}
.dwxg-viewport{position:relative;flex:1;min-height:180px;overflow:auto;background:var(--dwxg-view-bg,#e7e9ed);overscroll-behavior:contain}
.dwxg-stage{position:relative;display:flex;align-items:center;justify-content:center;min-width:100%;min-height:100%;box-sizing:border-box;padding:0}
.dwxg-stage canvas{display:block;flex:none;image-rendering:pixelated;image-rendering:crisp-edges;max-width:none;max-height:none}
.dwxg-status{padding:6px 9px;border-top:1px solid #e2e4e8;font-size:12px;min-height:1em;color:#4e5661}
.dwxg-hidden-input{display:none}
`;

function ensureCss(doc) {
  if (doc.getElementById('dwxg-viewer-css')) return;
  const style = doc.createElement('style');
  style.id = 'dwxg-viewer-css';
  style.textContent = CSS;
  doc.head.append(style);
}

/**
 * Mount a file-opening DWXG viewer into a DOM element.
 * Returns { openFile(file), load(input), destroy(), getImage() }.
 */
export function mountDwxgViewer(host, options = {}) {
  if (!host || !host.ownerDocument) throw new TypeError('mountDwxgViewer(host) requires a DOM element.');
  const doc = host.ownerDocument;
  ensureCss(doc);
  host.replaceChildren();
  const root = doc.createElement('section'); root.className = 'dwxg-viewer';
  const toolbar = doc.createElement('div'); toolbar.className = 'dwxg-toolbar';
  const openButton = doc.createElement('button'); openButton.type = 'button'; openButton.textContent = 'Open DWXG…';
  const fileInput = doc.createElement('input'); fileInput.className = 'dwxg-hidden-input'; fileInput.type = 'file'; fileInput.accept = '.dwxg,application/octet-stream';
  const fitButton = doc.createElement('button'); fitButton.type = 'button'; fitButton.textContent = 'Fit';
  const actualButton = doc.createElement('button'); actualButton.type = 'button'; actualButton.textContent = '100%';
  const zoomLabel = doc.createElement('label'); zoomLabel.textContent = 'Zoom ';
  const zoomInput = doc.createElement('input'); zoomInput.type = 'range'; zoomInput.min = '0.1'; zoomInput.max = '10'; zoomInput.step = '0.1'; zoomInput.value = '1'; zoomLabel.append(zoomInput);
  const zoomValue = doc.createElement('span'); zoomValue.textContent = '100%';
  const meta = doc.createElement('span'); meta.className = 'dwxg-meta'; meta.textContent = 'Open a .dwxg file to view it.';
  toolbar.append(openButton, fileInput, fitButton, actualButton, zoomLabel, zoomValue, meta);
  const viewport = doc.createElement('div'); viewport.className = 'dwxg-viewport'; viewport.tabIndex = 0; viewport.setAttribute('aria-label', 'DWXG image viewport. Use the mouse wheel to zoom and scrollbars to pan.');
  const stage = doc.createElement('div'); stage.className = 'dwxg-stage';
  const canvas = doc.createElement('canvas'); canvas.width = 1; canvas.height = 1;
  stage.append(canvas); viewport.append(stage);
  const status = doc.createElement('div'); status.className = 'dwxg-status'; status.setAttribute('role', 'status'); status.textContent = 'Ready.';
  root.append(toolbar, viewport, status); host.append(root);

  const state = { image: null, zoom: 1, bytes: 0, fileName: '', destroyed: false };
  const minZoom = 0.1;
  const maxZoom = 10;

  function updateGeometry() {
    if (!state.image) return;
    const w = Math.max(1, Math.round(state.image.width * state.zoom));
    const h = Math.max(1, Math.round(state.image.height * state.zoom));
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    stage.style.width = `${Math.max(viewport.clientWidth, w)}px`;
    stage.style.height = `${Math.max(viewport.clientHeight, h)}px`;
    zoomInput.value = String(state.zoom);
    zoomValue.textContent = `${Math.round(state.zoom * 100)}%`;
  }

  function setZoom(nextZoom, anchorEvent = null) {
    if (!state.image) return;
    const next = Math.min(maxZoom, Math.max(minZoom, nextZoom));
    if (Math.abs(next - state.zoom) < 0.0001) return;
    let pointerX = viewport.clientWidth / 2;
    let pointerY = viewport.clientHeight / 2;
    let imageX = state.image.width / 2;
    let imageY = state.image.height / 2;
    if (anchorEvent) {
      const oldRect = canvas.getBoundingClientRect();
      pointerX = anchorEvent.clientX - viewport.getBoundingClientRect().left;
      pointerY = anchorEvent.clientY - viewport.getBoundingClientRect().top;
      imageX = (anchorEvent.clientX - oldRect.left) / state.zoom;
      imageY = (anchorEvent.clientY - oldRect.top) / state.zoom;
    }
    state.zoom = next;
    updateGeometry();
    // Preserve the source pixel under the pointer after changing scale.
    const newRect = canvas.getBoundingClientRect();
    const viewportRect = viewport.getBoundingClientRect();
    const desiredScreenX = viewportRect.left + pointerX;
    const desiredScreenY = viewportRect.top + pointerY;
    const currentPointX = newRect.left + imageX * state.zoom;
    const currentPointY = newRect.top + imageY * state.zoom;
    viewport.scrollLeft += currentPointX - desiredScreenX;
    viewport.scrollTop += currentPointY - desiredScreenY;
  }

  function fitToView() {
    if (!state.image) return;
    const availableW = Math.max(1, viewport.clientWidth - 24);
    const availableH = Math.max(1, viewport.clientHeight - 24);
    state.zoom = Math.min(1, availableW / state.image.width, availableH / state.image.height);
    state.zoom = Math.min(maxZoom, Math.max(minZoom, state.zoom));
    updateGeometry();
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
  }

  function drawImage(image) {
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext('2d', { alpha: false });
    context.imageSmoothingEnabled = false;
    // decodeDwxg() intentionally returns compact RGB (3 bytes per pixel),
    // while Canvas ImageData requires RGBA (4 bytes per pixel). Expand the
    // channels and set alpha to opaque before handing the buffer to Canvas.
    const rgba = new Uint8ClampedArray(image.width * image.height * 4);
    for (let src = 0, dst = 0; src < image.rgb.length; src += 3, dst += 4) {
      rgba[dst] = image.rgb[src];
      rgba[dst + 1] = image.rgb[src + 1];
      rgba[dst + 2] = image.rgb[src + 2];
      rgba[dst + 3] = 255;
    }
    context.putImageData(new ImageData(rgba, image.width, image.height), 0, 0);
    state.image = image;
    state.zoom = Math.min(1, 800 / image.width, 520 / image.height);
    state.zoom = Math.max(minZoom, state.zoom);
    updateGeometry();
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
    meta.textContent = `${image.width} × ${image.height} · ${image.bitsPerPixel}-bit${image.legacy ? ' · legacy DWXG' : ''}${image.compressed ? ' · zlib' : ''}`;
    status.textContent = `${state.fileName || 'DWXG image'} · ${state.bytes.toLocaleString()} bytes`;
    fitToView();
  }

  async function load(input, info = {}) {
    status.textContent = 'Reading DWXG…';
    try {
      const bytes = bytesOf(input);
      const image = await decodeDwxg(bytes, options);
      state.bytes = info.size ?? bytes.byteLength;
      state.fileName = info.name ?? '';
      drawImage(image);
      return image;
    } catch (error) {
      status.textContent = error.message || String(error);
      throw error;
    }
  }

  async function openFile(file) {
    if (!file) return null;
    try {
      const image = await load(await file.arrayBuffer(), { name: file.name, size: file.size });
      return image;
    } catch {
      return null; // Error is visible in the status line; avoid an unhandled click promise.
    }
  }

  openButton.addEventListener('click', () => fileInput.click());
  fileInput.addEventListener('change', () => { if (fileInput.files?.[0]) void openFile(fileInput.files[0]); });
  fitButton.addEventListener('click', fitToView);
  actualButton.addEventListener('click', () => { state.zoom = 1; updateGeometry(); viewport.scrollLeft = 0; viewport.scrollTop = 0; });
  zoomInput.addEventListener('input', () => setZoom(Number(zoomInput.value)));
  viewport.addEventListener('wheel', event => {
    if (!state.image) return;
    event.preventDefault();
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    setZoom(state.zoom * factor, event);
  }, { passive: false });
  const onResize = () => {
    if (!state.image) return;
    const oldLeft = viewport.scrollLeft;
    const oldTop = viewport.scrollTop;
    updateGeometry();
    viewport.scrollLeft = oldLeft;
    viewport.scrollTop = oldTop;
  };
  const resizeObserver = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(onResize) : null;
  resizeObserver?.observe(viewport);

  return {
    openFile,
    load,
    destroy() {
      state.destroyed = true;
      resizeObserver?.disconnect();
      root.remove();
    },
    getImage() { return state.image; },
    get zoom() { return state.zoom; },
  };
}
