import createFontdone from "../index.js";

const input = document.querySelector("#font");
const status = document.querySelector("#status");
const canvas = document.querySelector("#glyph");
const context = canvas.getContext("2d");
if (context === null) {
  throw new Error("this browser does not provide a 2D canvas context");
}
const enginePromise = createFontdone();

function reportError(error) {
  status.textContent = error instanceof Error ? error.message : String(error);
  document.documentElement.dataset.fontdoneState = "error";
}

async function render(fontBytes) {
  status.textContent = "Rendering…";
  const engine = await enginePromise;
  const face = engine.openFace(fontBytes, { pixelSize: 48 });
  try {
    const bitmap = face.render("A");
    const scale = 4;
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    canvas.style.width = `${bitmap.width * scale}px`;
    canvas.style.height = `${bitmap.height * scale}px`;

    const image = context.createImageData(bitmap.width, bitmap.height);
    const stride = Math.abs(bitmap.pitch);
    for (let y = 0; y < bitmap.height; y += 1) {
      const sourceY = bitmap.pitch < 0 ? bitmap.height - y - 1 : y;
      for (let x = 0; x < bitmap.width; x += 1) {
        const coverage = bitmap.pixels[sourceY * stride + x];
        const destination = (y * bitmap.width + x) * 4;
        image.data[destination] = 0;
        image.data[destination + 1] = 0;
        image.data[destination + 2] = 0;
        image.data[destination + 3] = coverage;
      }
    }
    context.putImageData(image, 0, 0);
    status.textContent =
      `Rendered glyph ${bitmap.glyphIndex}: ${bitmap.width}×${bitmap.height}, ` +
      `${bitmap.pixels.length} bytes`;
    document.documentElement.dataset.fontdoneState = "rendered";
  } finally {
    face.close();
  }
}

input.addEventListener("change", async () => {
  const [file] = input.files;
  if (file) {
    try {
      await render(await file.arrayBuffer());
    } catch (error) {
      reportError(error);
    }
  }
});

try {
  await enginePromise;
  status.textContent = "fontdone is ready; select a font.";
  document.documentElement.dataset.fontdoneState = "ready";
  const fontUrl = new URL(location.href).searchParams.get("font");
  if (fontUrl !== null) {
    const response = await fetch(fontUrl);
    if (!response.ok) {
      throw new Error(`cannot fetch test font: HTTP ${response.status}`);
    }
    await render(await response.arrayBuffer());
  }
} catch (error) {
  reportError(error);
  throw error;
}
