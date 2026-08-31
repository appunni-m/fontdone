const DEFAULT_WASM_URL = new URL("./fontdone.wasm", import.meta.url);

const REQUIRED_EXPORTS = [
  "fontdone_wasm_malloc",
  "fontdone_wasm_free",
  "fontdone_wasm_open_face_handle",
  "fontdone_wasm_done_face",
  "fontdone_wasm_set_pixel_sizes",
  "fontdone_wasm_get_char_index",
  "fontdone_wasm_load_glyph",
  "fontdone_wasm_render_glyph",
  "fontdone_wasm_bitmap_buffer",
  "fontdone_wasm_bitmap_len",
  "fontdone_wasm_bitmap_width",
  "fontdone_wasm_bitmap_rows",
  "fontdone_wasm_bitmap_pitch",
];

/** Load flags promoted by the browser wrapper. Additional FreeType-compatible
 * bit flags may be passed numerically to `loadGlyph` or `render`. */
export const LoadFlags = Object.freeze({
  DEFAULT: 0,
});

/** Render modes promoted by the browser wrapper. */
export const RenderMode = Object.freeze({
  NORMAL: 0,
});

/** An error returned by the fontdone Wasm ABI. */
export class FontdoneError extends Error {
  /**
   * @param {string} operation operation that failed
   * @param {number} code FreeType-compatible `FT_Error` value
   */
  constructor(operation, code) {
    super(`${operation} failed with FT_Error ${code}`);
    this.name = "FontdoneError";
    this.operation = operation;
    this.code = code;
  }
}

function assertInteger(value, name, minimum, maximum) {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(
      `${name} must be an integer from ${minimum} through ${maximum}`,
    );
  }
  return value;
}

function asU32(value, name) {
  return assertInteger(value, name, 0, 0xffff_ffff);
}

function asI32Bits(value, name) {
  return assertInteger(value, name, -0x8000_0000, 0xffff_ffff);
}

function asFaceIndex(value) {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new RangeError("faceIndex must be a safe integer or bigint");
    }
    value = BigInt(value);
  }
  if (
    typeof value !== "bigint" ||
    value < -0x8000_0000_0000_0000n ||
    value > 0x7fff_ffff_ffff_ffffn
  ) {
    throw new RangeError("faceIndex must fit in a signed 64-bit integer");
  }
  return value;
}

function asFontBytes(value) {
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  throw new TypeError("fontData must be an ArrayBuffer or an ArrayBuffer view");
}

function asCodePoint(value) {
  if (typeof value === "string") {
    const scalars = [...value];
    if (scalars.length !== 1) {
      throw new TypeError("character must contain exactly one Unicode scalar");
    }
    value = scalars[0].codePointAt(0);
  }
  value = assertInteger(value, "codePoint", 0, 0x10_ffff);
  if (value >= 0xd800 && value <= 0xdfff) {
    throw new RangeError("codePoint must be a Unicode scalar value");
  }
  return value;
}

function assertExports(exports) {
  if (!(exports.memory instanceof WebAssembly.Memory)) {
    throw new TypeError("fontdone Wasm module does not export memory");
  }
  for (const name of REQUIRED_EXPORTS) {
    if (typeof exports[name] !== "function") {
      throw new TypeError(`fontdone Wasm module is missing export ${name}`);
    }
  }
  return exports;
}

function responseLike(value) {
  return typeof Response !== "undefined" && value instanceof Response;
}

function requestLike(value) {
  return typeof Request !== "undefined" && value instanceof Request;
}

async function instantiateResponse(response) {
  if (!response.ok) {
    throw new TypeError(
      `cannot fetch fontdone Wasm: HTTP ${response.status} ${response.statusText}`,
    );
  }

  if (typeof WebAssembly.instantiateStreaming === "function") {
    const fallback = response.clone();
    try {
      const result = await WebAssembly.instantiateStreaming(response, {});
      return result.instance;
    } catch (streamingError) {
      try {
        const result = await WebAssembly.instantiate(
          await fallback.arrayBuffer(),
          {},
        );
        return result.instance;
      } catch (error) {
        if (error instanceof Error && error.cause === undefined) {
          error.cause = streamingError;
        }
        throw error;
      }
    }
  }

  const result = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  return result.instance;
}

async function instantiateSource(source) {
  source = await source;
  if (source instanceof WebAssembly.Instance) {
    return source;
  }
  if (source instanceof WebAssembly.Module) {
    return WebAssembly.instantiate(source, {});
  }
  if (responseLike(source)) {
    return instantiateResponse(source);
  }
  if (
    typeof source === "string" ||
    source instanceof URL ||
    requestLike(source)
  ) {
    return instantiateResponse(await fetch(source));
  }
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    const result = await WebAssembly.instantiate(source, {});
    return result.instance;
  }
  throw new TypeError(
    "Wasm source must be a URL, Request, Response, BufferSource, Module, or Instance",
  );
}

function throwForError(operation, code) {
  if (code !== 0) {
    throw new FontdoneError(operation, code);
  }
}

class FontdoneFaceImpl {
  #exports;
  #handle;
  #glyphIndex = 0;
  #onClose;

  constructor(exports, handle, onClose) {
    this.#exports = exports;
    this.#handle = handle;
    this.#onClose = onClose;
  }

  get closed() {
    return this.#handle === 0;
  }

  #assertOpen() {
    if (this.closed) {
      throw new Error("fontdone face is closed");
    }
  }

  setPixelSize(size) {
    return this.setPixelSizes(0, size);
  }

  setPixelSizes(pixelWidth, pixelHeight) {
    this.#assertOpen();
    pixelWidth = asU32(pixelWidth, "pixelWidth");
    pixelHeight = asU32(pixelHeight, "pixelHeight");
    if (pixelWidth === 0 && pixelHeight === 0) {
      throw new RangeError("pixelWidth and pixelHeight cannot both be zero");
    }
    throwForError(
      "setPixelSizes",
      this.#exports.fontdone_wasm_set_pixel_sizes(
        this.#handle,
        pixelWidth,
        pixelHeight,
      ),
    );
    this.#glyphIndex = 0;
    return this;
  }

  getCharIndex(character) {
    this.#assertOpen();
    const codePoint = asCodePoint(character);
    return this.#exports.fontdone_wasm_get_char_index(
      this.#handle,
      BigInt(codePoint),
    );
  }

  loadGlyph(glyphIndex, loadFlags = LoadFlags.DEFAULT) {
    this.#assertOpen();
    glyphIndex = asU32(glyphIndex, "glyphIndex");
    loadFlags = asI32Bits(loadFlags, "loadFlags");
    throwForError(
      "loadGlyph",
      this.#exports.fontdone_wasm_load_glyph(
        this.#handle,
        glyphIndex,
        loadFlags,
      ),
    );
    this.#glyphIndex = glyphIndex;
    return this;
  }

  renderGlyph(renderMode = RenderMode.NORMAL) {
    this.#assertOpen();
    renderMode = assertInteger(
      renderMode,
      "renderMode",
      -0x8000_0000,
      0x7fff_ffff,
    );
    throwForError(
      "renderGlyph",
      this.#exports.fontdone_wasm_render_glyph(this.#handle, renderMode),
    );
    return this.#copyBitmap();
  }

  render(character, options = {}) {
    const glyphIndex = this.getCharIndex(character);
    this.loadGlyph(glyphIndex, options.loadFlags ?? LoadFlags.DEFAULT);
    return this.renderGlyph(options.renderMode ?? RenderMode.NORMAL);
  }

  #copyBitmap() {
    const width = this.#exports.fontdone_wasm_bitmap_width(this.#handle);
    const height = this.#exports.fontdone_wasm_bitmap_rows(this.#handle);
    const pitch = this.#exports.fontdone_wasm_bitmap_pitch(this.#handle);
    const pointer = this.#exports.fontdone_wasm_bitmap_buffer(this.#handle);
    const length = this.#exports.fontdone_wasm_bitmap_len(this.#handle);
    const expectedLength = Math.abs(pitch) * height;

    if (length !== expectedLength) {
      throw new Error(
        `fontdone returned inconsistent bitmap length ${length}; expected ${expectedLength}`,
      );
    }
    if (length !== 0 && pointer === 0) {
      throw new Error("fontdone returned a null bitmap pointer with non-zero length");
    }
    if (pointer + length > this.#exports.memory.buffer.byteLength) {
      throw new Error("fontdone returned a bitmap outside exported memory");
    }

    const pixels = new Uint8Array(
      this.#exports.memory.buffer,
      pointer,
      length,
    ).slice();
    return Object.freeze({
      glyphIndex: this.#glyphIndex,
      width,
      height,
      pitch,
      pixels,
    });
  }

  close() {
    if (this.closed) {
      return;
    }
    const handle = this.#handle;
    this.#handle = 0;
    this.#onClose();
    throwForError(
      "closeFace",
      this.#exports.fontdone_wasm_done_face(handle),
    );
  }
}

class FontdoneImpl {
  #exports;
  #faces = new Set();
  #closed = false;

  constructor(exports) {
    this.#exports = exports;
  }

  get closed() {
    return this.#closed;
  }

  openFace(fontData, options = {}) {
    if (this.closed) {
      throw new Error("fontdone instance is closed");
    }
    const bytes = asFontBytes(fontData);
    if (bytes.byteLength > 0xffff_ffff) {
      throw new RangeError("fontData is too large for wasm32 linear memory");
    }
    const faceIndex = asFaceIndex(options.faceIndex ?? 0);
    const pixelSize = asU32(options.pixelSize ?? 16, "pixelSize");
    if (pixelSize === 0) {
      throw new RangeError("pixelSize must be greater than zero");
    }

    const errorPointer = this.#exports.fontdone_wasm_malloc(4);
    if (errorPointer === 0) {
      throw new Error("fontdone could not allocate the face error output");
    }

    let fontPointer = 0;
    let handle = 0;
    try {
      fontPointer = this.#exports.fontdone_wasm_malloc(bytes.byteLength);
      if (fontPointer === 0) {
        throw new Error("fontdone could not allocate the font input");
      }
      new Uint8Array(
        this.#exports.memory.buffer,
        fontPointer,
        bytes.byteLength,
      ).set(bytes);

      handle = this.#exports.fontdone_wasm_open_face_handle(
        fontPointer,
        bytes.byteLength,
        faceIndex,
        pixelSize,
        errorPointer,
      );
      const code = new DataView(this.#exports.memory.buffer).getInt32(
        errorPointer,
        true,
      );
      if (code !== 0) {
        throw new FontdoneError("openFace", code);
      }
      if (handle === 0) {
        throw new Error("fontdone opened a face without returning a handle");
      }
      throwForError(
        "setPixelSizes",
        this.#exports.fontdone_wasm_set_pixel_sizes(handle, 0, pixelSize),
      );

      let face;
      face = new FontdoneFaceImpl(this.#exports, handle, () => {
        this.#faces.delete(face);
      });
      this.#faces.add(face);
      handle = 0;
      return face;
    } finally {
      if (handle !== 0) {
        this.#exports.fontdone_wasm_done_face(handle);
      }
      if (fontPointer !== 0) {
        this.#exports.fontdone_wasm_free(fontPointer, bytes.byteLength);
      }
      this.#exports.fontdone_wasm_free(errorPointer, 4);
    }
  }

  close() {
    if (this.closed) {
      return;
    }
    let firstError;
    for (const face of [...this.#faces]) {
      try {
        face.close();
      } catch (error) {
        firstError ??= error;
      }
    }
    this.#closed = true;
    if (firstError !== undefined) {
      throw firstError;
    }
  }
}

/**
 * Instantiates a fresh fontdone engine. When `source` is omitted, the package's
 * `fontdone.wasm` asset is fetched relative to this module.
 *
 * Each call creates an independent WebAssembly instance. Face handles and
 * memory offsets never cross between instances.
 *
 * @param {unknown} [source] URL, response, bytes, compiled module, or instance
 * @returns {Promise<FontdoneImpl>}
 */
export async function createFontdone(source = DEFAULT_WASM_URL) {
  const instance = await instantiateSource(source);
  return new FontdoneImpl(assertExports(instance.exports));
}

/** Alias matching the initialization convention used by many Wasm packages. */
export const init = createFontdone;

export default createFontdone;
