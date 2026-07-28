#include "fontdone_ffi.h"

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>

static int read_file(const char *path, unsigned char **bytes, size_t *length) {
  FILE *file = fopen(path, "rb");
  long end;
  unsigned char *buffer;

  if (file == NULL) {
    perror(path);
    return 0;
  }
  if (fseek(file, 0, SEEK_END) != 0 || (end = ftell(file)) < 0 ||
      fseek(file, 0, SEEK_SET) != 0) {
    perror("seek font");
    fclose(file);
    return 0;
  }
  if ((unsigned long)end > (unsigned long)LONG_MAX) {
    fprintf(stderr, "font is too large for FT_Long\n");
    fclose(file);
    return 0;
  }
  buffer = malloc(end == 0 ? 1U : (size_t)end);
  if (buffer == NULL) {
    fprintf(stderr, "cannot allocate font buffer\n");
    fclose(file);
    return 0;
  }
  if (end != 0 && fread(buffer, 1U, (size_t)end, file) != (size_t)end) {
    perror("read font");
    free(buffer);
    fclose(file);
    return 0;
  }
  fclose(file);
  *bytes = buffer;
  *length = (size_t)end;
  return 1;
}

static int check_error(const char *operation, FT_Error error) {
  if (error == FT_Err_Ok) {
    return 1;
  }
  fprintf(stderr, "%s failed with FT_Error %d\n", operation, error);
  return 0;
}

int main(int argc, char **argv) {
  unsigned char *font_bytes = NULL;
  size_t font_length = 0;
  FT_Library library = NULL;
  FT_Face face = NULL;
  FT_UInt glyph_index;
  const FT_Bitmap *bitmap;
  size_t bitmap_length;
  int result = EXIT_FAILURE;

  if (argc != 2) {
    fprintf(stderr, "usage: %s FONT_FILE\n", argv[0]);
    return EXIT_FAILURE;
  }
  if (!read_file(argv[1], &font_bytes, &font_length)) {
    return EXIT_FAILURE;
  }
  if (!check_error("FT_Init_FreeType", FT_Init_FreeType(&library))) {
    goto cleanup;
  }
  if (font_length > (size_t)LONG_MAX ||
      !check_error("FT_New_Memory_Face",
                   FT_New_Memory_Face(library, font_bytes,
                                      (FT_Long)font_length, 0, &face))) {
    goto cleanup;
  }

  /* fontdone copies memory-face bytes during open. */
  free(font_bytes);
  font_bytes = NULL;

  if (!check_error("FT_Set_Pixel_Sizes",
                   FT_Set_Pixel_Sizes(face, 0U, 16U))) {
    goto cleanup;
  }
  glyph_index = FT_Get_Char_Index(face, (FT_ULong)'A');
  if (glyph_index == 0U) {
    fprintf(stderr, "font has no glyph for U+0041\n");
    goto cleanup;
  }
  if (!check_error("FT_Load_Glyph",
                   FT_Load_Glyph(face, glyph_index,
                                 FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL))) {
    goto cleanup;
  }
  if (face->glyph == NULL) {
    fprintf(stderr, "successful load returned no glyph slot\n");
    goto cleanup;
  }
  bitmap = &face->glyph->bitmap;
  bitmap_length = (size_t)(bitmap->pitch < 0 ? -bitmap->pitch : bitmap->pitch) *
                  (size_t)bitmap->rows;
  if (bitmap_length != 0U && bitmap->buffer == NULL) {
    fprintf(stderr, "rendered bitmap has no buffer\n");
    goto cleanup;
  }
  printf("glyph=%u bitmap=%ux%u pitch=%d bytes=%zu advance=%ld\n",
         glyph_index, bitmap->width, bitmap->rows, bitmap->pitch,
         bitmap_length, (long)face->glyph->advance.x);
  result = EXIT_SUCCESS;

cleanup:
  free(font_bytes);
  if (face != NULL && !check_error("FT_Done_Face", FT_Done_Face(face))) {
    result = EXIT_FAILURE;
  }
  if (library != NULL &&
      !check_error("FT_Done_FreeType", FT_Done_FreeType(library))) {
    result = EXIT_FAILURE;
  }
  return result;
}
