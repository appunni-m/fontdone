# Compile optional validation modules into the pinned oracle library without
# registering them in FreeType's default 19-module list.  Validator-specific
# oracle cases opt in with `FT_Add_Module`, preserving both enabled and
# disabled behavior from one version-pinned build.

if(CMAKE_VERSION VERSION_LESS 3.19)
  message(FATAL_ERROR "fontdone validator oracle support requires CMake 3.19 or newer")
endif()

set(FONTDONE_ORACLE_VALIDATOR_HELPER
    "${CMAKE_CURRENT_BINARY_DIR}/fontdone_oracle_validator_helper.c")

file(WRITE "${FONTDONE_ORACLE_VALIDATOR_HELPER}" [=[
#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_MODULE_H
#include <freetype/internal/ftobjs.h>

extern const FT_Module_Class otv_module_class;
extern const FT_Module_Class gxv_module_class;

FT_EXPORT( FT_Error )
fontdone_oracle_add_otvalid( FT_Library library );
FT_EXPORT( FT_Error )
fontdone_oracle_add_gxvalid( FT_Library library );

FT_EXPORT_DEF( FT_Error )
fontdone_oracle_add_otvalid( FT_Library library )
{
  return FT_Add_Module( library, &otv_module_class );
}

FT_EXPORT_DEF( FT_Error )
fontdone_oracle_add_gxvalid( FT_Library library )
{
  return FT_Add_Module( library, &gxv_module_class );
}
]=])

cmake_language(
  DEFER
  CALL target_sources freetype PRIVATE
       "${CMAKE_SOURCE_DIR}/src/otvalid/otvalid.c"
       "${CMAKE_SOURCE_DIR}/src/gxvalid/gxvalid.c"
       "${FONTDONE_ORACLE_VALIDATOR_HELPER}"
)
