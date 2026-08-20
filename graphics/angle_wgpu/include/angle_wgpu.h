#ifndef ANGLE_WGPU_H
#define ANGLE_WGPU_H

#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
typedef void GLvoid;


/**
 * Host window description for `angle_wgpu_create_native_window_surface`.
 * Do not pass this to `eglCreateWindowSurface`; that entry point is headless.
 */
#define ANGLE_WGPU_NATIVE_X11 1

#define ANGLE_WGPU_NATIVE_WAYLAND 2

#define ANGLE_WGPU_NATIVE_WIN32 3

typedef struct WinitApp WinitApp;

typedef uint32_t GLenum;

typedef float GLfloat;

typedef double GLdouble;

typedef int32_t GLint;

typedef int32_t GLsizei;

typedef uint8_t GLubyte;

typedef uint32_t GLuint;

typedef uint8_t GLboolean;

typedef float GLclampf;

typedef int32_t GLfixed;

typedef double GLclampd;

typedef int32_t GLclampx;

typedef uint32_t GLbitfield;

typedef char GLchar;

typedef ptrdiff_t GLsizeiptr;

typedef ptrdiff_t GLintptr;

typedef void *EGLDisplay;

typedef void *NativeDisplayType;

typedef uint32_t EGLBoolean;

typedef int32_t EGLint;

typedef void *EGLConfig;

typedef void *EGLSurface;

typedef void *NativeWindowType;

typedef struct AngleWgpuNativeWindow {
    /**
     * 1 = X11, 2 = Wayland, 3 = Win32.
     */
    uint32_t kind;
    /**
     * X11 `Display*`, Wayland `wl_display*`, or Win32 `HINSTANCE`.
     */
    void *display;
    /**
     * X11 `Window` id, Wayland `wl_surface*`, or Win32 `HWND`.
     */
    uint64_t window;
    uint32_t width;
    uint32_t height;
    /**
     * X11 screen number; unused on other backends.
     */
    int32_t screen;
} AngleWgpuNativeWindow;

typedef uint32_t EGLenum;

typedef void *EGLContext;

typedef void (*__eglMustCastToProperFunctionPointerType)(void);

typedef struct DecodedImage {
    uint32_t *pixels;
    int32_t width;
    int32_t height;
} DecodedImage;

#define EGL_NO_DISPLAY (EGLDisplay)0

#define EGL_NO_CONTEXT (EGLContext)0

#define EGL_NO_SURFACE (EGLSurface)0

#define EGL_DEFAULT_DISPLAY (EGLDisplay)0

#define EGL_FALSE 0

#define EGL_TRUE 1

#define EGL_SUCCESS 12288

#define EGL_NOT_INITIALIZED 12289

#define EGL_BAD_ACCESS 12290

#define EGL_BAD_ALLOC 12291

#define EGL_BAD_ATTRIBUTE 12292

#define EGL_BAD_CONFIG 12293

#define EGL_BAD_CONTEXT 12294

#define EGL_BAD_CURRENT_SURFACE 12295

#define EGL_BAD_DISPLAY 12296

#define EGL_BAD_MATCH 12297

#define EGL_BAD_NATIVE_PIXMAP 12298

#define EGL_BAD_NATIVE_WINDOW 12299

#define EGL_BAD_PARAMETER 12300

#define EGL_BAD_SURFACE 12301

#define EGL_CONTEXT_LOST 12302

#define EGL_BUFFER_SIZE 12320

#define EGL_ALPHA_SIZE 12321

#define EGL_BLUE_SIZE 12322

#define EGL_GREEN_SIZE 12323

#define EGL_RED_SIZE 12324

#define EGL_DEPTH_SIZE 12325

#define EGL_STENCIL_SIZE 12326

#define EGL_CONFIG_CAVEAT 12327

#define EGL_CONFIG_ID 12328

#define EGL_LEVEL 12329

#define EGL_MAX_PBUFFER_HEIGHT 12330

#define EGL_MAX_PBUFFER_PIXELS 12331

#define EGL_MAX_PBUFFER_WIDTH 12332

#define EGL_NATIVE_RENDERABLE 12333

#define EGL_NATIVE_VISUAL_ID 12334

#define EGL_NATIVE_VISUAL_TYPE 12335

#define EGL_SAMPLES 12337

#define EGL_SAMPLE_BUFFERS 12338

#define EGL_SURFACE_TYPE 12339

#define EGL_TRANSPARENT_TYPE 12340

#define EGL_TRANSPARENT_BLUE_VALUE 12341

#define EGL_TRANSPARENT_GREEN_VALUE 12342

#define EGL_TRANSPARENT_RED_VALUE 12343

#define EGL_NONE 12344

#define EGL_BIND_TO_TEXTURE_RGB 12345

#define EGL_BIND_TO_TEXTURE_RGBA 12346

#define EGL_MIN_SWAP_INTERVAL 12347

#define EGL_MAX_SWAP_INTERVAL 12348

#define EGL_LUMINANCE_SIZE 12349

#define EGL_ALPHA_MASK_SIZE 12350

#define EGL_COLOR_BUFFER_TYPE 12351

#define EGL_RENDERABLE_TYPE 12352

#define EGL_CONFORMANT 12354

#define EGL_SLOW_CONFIG 12368

#define EGL_NON_CONFORMANT_CONFIG 12369

#define EGL_TRANSPARENT_RGB 12370

#define EGL_RGB_BUFFER 12430

#define EGL_LUMINANCE_BUFFER 12431

#define EGL_PBUFFER_BIT 1

#define EGL_PIXMAP_BIT 2

#define EGL_WINDOW_BIT 4

#define EGL_OPENGL_ES_BIT 1

#define EGL_OPENVG_BIT 2

#define EGL_OPENGL_ES2_BIT 4

#define EGL_OPENGL_BIT 8

#define EGL_VENDOR 12371

#define EGL_VERSION 12372

#define EGL_EXTENSIONS 12373

#define EGL_CLIENT_APIS 12429

#define EGL_HEIGHT 12374

#define EGL_WIDTH 12375

#define EGL_LARGEST_PBUFFER 12376

#define EGL_TEXTURE_FORMAT 12416

#define EGL_TEXTURE_TARGET 12417

#define EGL_MIPMAP_TEXTURE 12418

#define EGL_MIPMAP_LEVEL 12419

#define EGL_BACK_BUFFER 12420

#define EGL_CONTEXT_CLIENT_VERSION 12440

#define EGL_OPENGL_ES_API 12448

#define EGL_OPENVG_API 12449

#define EGL_OPENGL_API 12450

#define GL_FALSE 0

#define GL_TRUE 1

#define GL_BYTE 5120

#define GL_UNSIGNED_BYTE 5121

#define GL_SHORT 5122

#define GL_UNSIGNED_SHORT 5123

#define GL_INT 5124

#define GL_UNSIGNED_INT 5125

#define GL_FLOAT 5126

#define GL_FIXED 5132

#define GL_UNSIGNED_SHORT_4_4_4_4 32819

#define GL_UNSIGNED_SHORT_5_5_5_1 32820

#define GL_UNSIGNED_SHORT_5_6_5 33635

#define GL_POINTS 0

#define GL_LINES 1

#define GL_LINE_LOOP 2

#define GL_LINE_STRIP 3

#define GL_TRIANGLES 4

#define GL_TRIANGLE_STRIP 5

#define GL_TRIANGLE_FAN 6

#define GL_QUADS 7

#define GL_QUAD_STRIP 8

#define GL_POLYGON 9

#define GL_NO_ERROR 0

#define GL_INVALID_ENUM 1280

#define GL_INVALID_VALUE 1281

#define GL_INVALID_OPERATION 1282

#define GL_MATRIX_MODE 2976

#define GL_STACK_OVERFLOW 1283

#define GL_STACK_UNDERFLOW 1284

#define GL_OUT_OF_MEMORY 1285

#define GL_INVALID_FRAMEBUFFER_OPERATION 1286

#define GL_NEVER 512

#define GL_LESS 513

#define GL_EQUAL 514

#define GL_LEQUAL 515

#define GL_GREATER 516

#define GL_NOTEQUAL 517

#define GL_GEQUAL 518

#define GL_ALWAYS 519

#define GL_ZERO 0

#define GL_ONE 1

#define GL_SRC_COLOR 768

#define GL_ONE_MINUS_SRC_COLOR 769

#define GL_SRC_ALPHA 770

#define GL_ONE_MINUS_SRC_ALPHA 771

#define GL_DST_ALPHA 772

#define GL_ONE_MINUS_DST_ALPHA 773

#define GL_DST_COLOR 774

#define GL_ONE_MINUS_DST_COLOR 775

#define GL_SRC_ALPHA_SATURATE 776

#define GL_CONSTANT_COLOR 32769

#define GL_ONE_MINUS_CONSTANT_COLOR 32770

#define GL_CONSTANT_ALPHA 32771

#define GL_ONE_MINUS_CONSTANT_ALPHA 32772

#define GL_FRONT 1028

#define GL_BACK 1029

#define GL_FRONT_AND_BACK 1032

#define GL_CW 2304

#define GL_CCW 2305

#define GL_CULL_FACE 2884

#define GL_LIGHTING 2896

#define GL_FOG 2912

#define GL_DEPTH_TEST 2929

#define GL_STENCIL_TEST 2960

#define GL_NORMALIZE 2977

#define GL_ALPHA_TEST 3008

#define GL_BLEND 3042

#define GL_COLOR_LOGIC_OP 3058

#define GL_SCISSOR_TEST 3089

#define GL_TEXTURE_2D 3553

#define GL_POLYGON_OFFSET_FILL 32823

#define GL_RESCALE_NORMAL 32826

#define GL_COLOR_MATERIAL 2903

#define GL_MODELVIEW 5888

#define GL_PROJECTION 5889

#define GL_TEXTURE 5890

#define GL_COLOR_BUFFER_BIT 16384

#define GL_DEPTH_BUFFER_BIT 256

#define GL_STENCIL_BUFFER_BIT 1024

#define GL_CURRENT_COLOR 2816

#define GL_CURRENT_NORMAL 2818

#define GL_CURRENT_TEXTURE_COORDS 2819

#define GL_LINE_WIDTH 2849

#define GL_CULL_FACE_MODE 2885

#define GL_FRONT_FACE 2886

#define GL_SHADE_MODEL 2900

#define GL_DEPTH_RANGE 2928

#define GL_DEPTH_WRITEMASK 2930

#define GL_DEPTH_CLEAR_VALUE 2931

#define GL_DEPTH_FUNC 2932

#define GL_STENCIL_CLEAR_VALUE 2961

#define GL_STENCIL_FUNC 2962

#define GL_STENCIL_VALUE_MASK 2963

#define GL_STENCIL_FAIL 2964

#define GL_STENCIL_PASS_DEPTH_FAIL 2965

#define GL_STENCIL_PASS_DEPTH_PASS 2966

#define GL_STENCIL_REF 2967

#define GL_STENCIL_WRITEMASK 2968

#define GL_VIEWPORT 2978

#define GL_MODELVIEW_STACK_DEPTH 2979

#define GL_PROJECTION_STACK_DEPTH 2980

#define GL_TEXTURE_STACK_DEPTH 2981

#define GL_MODELVIEW_MATRIX 2982

#define GL_PROJECTION_MATRIX 2983

#define GL_TEXTURE_MATRIX 2984

#define GL_ALPHA_TEST_FUNC 3009

#define GL_ALPHA_TEST_REF 3010

#define GL_BLEND_DST 3040

#define GL_BLEND_SRC 3041

#define GL_LOGIC_OP_MODE 3056

#define GL_SCISSOR_BOX 3088

#define GL_COLOR_CLEAR_VALUE 3106

#define GL_COLOR_WRITEMASK 3107

#define GL_MAX_TEXTURE_SIZE 3379

#define GL_MAX_VIEWPORT_DIMS 3386

#define GL_SUBPIXEL_BITS 3408

#define GL_RED_BITS 3410

#define GL_GREEN_BITS 3411

#define GL_BLUE_BITS 3412

#define GL_ALPHA_BITS 3413

#define GL_DEPTH_BITS 3414

#define GL_STENCIL_BITS 3415

#define GL_TEXTURE_BINDING_2D 32873

#define GL_CURRENT_PROGRAM 35725

#define GL_VENDOR 7936

#define GL_RENDERER 7937

#define GL_VERSION 7938

#define GL_EXTENSIONS 7939

#define GL_NEAREST 9728

#define GL_LINEAR 9729

#define GL_NEAREST_MIPMAP_NEAREST 9984

#define GL_LINEAR_MIPMAP_NEAREST 9985

#define GL_NEAREST_MIPMAP_LINEAR 9986

#define GL_LINEAR_MIPMAP_LINEAR 9987

#define GL_TEXTURE_MAG_FILTER 10240

#define GL_TEXTURE_MIN_FILTER 10241

#define GL_TEXTURE_WRAP_S 10242

#define GL_TEXTURE_WRAP_T 10243

#define GL_TEXTURE_WIDTH 4096

#define GL_TEXTURE_HEIGHT 4097

#define GL_TEXTURE_INTERNAL_FORMAT 4099

#define GL_CLAMP 10496

#define GL_REPEAT 10497

#define GL_CLAMP_TO_EDGE 33071

#define GL_MIRRORED_REPEAT 33648

#define GL_S 8192

#define GL_T 8193

#define GL_R 8194

#define GL_Q 8195

#define GL_TEXTURE_GEN_MODE 9472

#define GL_OBJECT_PLANAR 9473

#define GL_EYE_PLANAR 9474

#define GL_EYE_LINEAR 9216

#define GL_OBJECT_LINEAR 9217

#define GL_SPHERE_MAP 9218

#define GL_TEXTURE_GEN_S 3168

#define GL_TEXTURE_GEN_T 3169

#define GL_TEXTURE_GEN_R 3170

#define GL_TEXTURE_GEN_Q 3171

#define GL_ALPHA 6406

#define GL_RGB 6407

#define GL_RGBA 6408

#define GL_LUMINANCE 6409

#define GL_LUMINANCE_ALPHA 6410

#define GL_BGRA_EXT 32993

#define GL_TEXTURE0 33984

#define GL_TEXTURE1 33985

#define GL_TEXTURE2 33986

#define GL_TEXTURE3 33987

#define GL_TEXTURE4 33988

#define GL_TEXTURE5 33989

#define GL_TEXTURE6 33990

#define GL_TEXTURE7 33991

#define GL_ACTIVE_TEXTURE 34016

#define GL_CLIENT_ACTIVE_TEXTURE 34017

#define GL_MAX_TEXTURE_UNITS 34018

#define GL_VERTEX_ARRAY 32884

#define GL_NORMAL_ARRAY 32885

#define GL_COLOR_ARRAY 32886

#define GL_TEXTURE_COORD_ARRAY 32888

#define GL_COMPILE 4864

#define GL_COMPILE_AND_EXECUTE 4865

#define GL_FLAT 7424

#define GL_SMOOTH 7425

#define GL_KEEP 7680

#define GL_REPLACE 7681

#define GL_INCR 7682

#define GL_DECR 7683

#define GL_INVERT 5386

#define GL_INCR_WRAP 34055

#define GL_DECR_WRAP 34056

#define GL_FOG_MODE 2917

#define GL_FOG_DENSITY 2914

#define GL_FOG_START 2915

#define GL_FOG_END 2916

#define GL_FOG_COLOR 2918

#define GL_EXP 2048

#define GL_EXP2 2049

#define GL_DONT_CARE 4352

#define GL_FASTEST 4353

#define GL_NICEST 4354

#define GL_PERSPECTIVE_CORRECTION_HINT 3152

#define GL_POINT_SMOOTH_HINT 3153

#define GL_LINE_SMOOTH_HINT 3154

#define GL_POLYGON_SMOOTH_HINT 3155

#define GL_FOG_HINT 3156

#define GL_GENERATE_MIPMAP_HINT 33170

#define GL_GENERATE_MIPMAP_HINT_SGIS 33170

#define GL_TEXTURE_COMPRESSION_HINT 34031

#define GL_FRAGMENT_SHADER_DERIVATIVE_HINT 35723

#define GL_FRAGMENT_SHADER_DERIVATIVE_HINT_OES 35723

#define GL_CLIP_VOLUME_CLIPPING_HINT_EXT 33008

#define GL_PHONG_HINT 33003

#define GL_PHONG_HINT_WIN 33003

#define GL_MULTISAMPLE_FILTER_HINT_NV 34100

#define GL_PROGRAM_BINARY_RETRIEVABLE_HINT 33367

#define GL_LIGHT0 16384

#define GL_LIGHT1 16385

#define GL_LIGHT2 16386

#define GL_LIGHT3 16387

#define GL_LIGHT4 16388

#define GL_LIGHT5 16389

#define GL_LIGHT6 16390

#define GL_LIGHT7 16391

#define GL_AMBIENT 4608

#define GL_DIFFUSE 4609

#define GL_SPECULAR 4610

#define GL_POSITION 4611

#define GL_SPOT_DIRECTION 4612

#define GL_SPOT_EXPONENT 4613

#define GL_SPOT_CUTOFF 4614

#define GL_CONSTANT_ATTENUATION 4615

#define GL_LINEAR_ATTENUATION 4616

#define GL_QUADRATIC_ATTENUATION 4617

#define GL_LIGHT_MODEL_AMBIENT 2899

#define GL_LIGHT_MODEL_TWO_SIDE 2898

#define GL_EMISSION 5632

#define GL_SHININESS 5633

#define GL_AMBIENT_AND_DIFFUSE 5634

#define GL_UNPACK_ALIGNMENT 3317

#define GL_PACK_ALIGNMENT 3333

#define GL_ARRAY_BUFFER 34962

#define GL_ELEMENT_ARRAY_BUFFER 34963

#define GL_ARRAY_BUFFER_BINDING 34964

#define GL_ELEMENT_ARRAY_BUFFER_BINDING 34965

#define GL_STATIC_DRAW 35044

#define GL_DYNAMIC_DRAW 35048

#define GL_STREAM_DRAW 35040

#define GL_FRAGMENT_SHADER 35632

#define GL_VERTEX_SHADER 35633

#define GL_COMPILE_STATUS 35713

#define GL_LINK_STATUS 35714

#define GL_VALIDATE_STATUS 35715

#define GL_INFO_LOG_LENGTH 35716

#define GL_ATTACHED_SHADERS 35717

#define GL_ACTIVE_UNIFORMS 35718

#define GL_ACTIVE_ATTRIBUTES 35721

#define GL_FRAMEBUFFER 36160

#define GL_RENDERBUFFER 36161

#define GL_COLOR_ATTACHMENT0 36064

#define GL_DEPTH_ATTACHMENT 36096

#define GL_STENCIL_ATTACHMENT 36128

#define GL_FRAMEBUFFER_COMPLETE 36053

#define GL_ALL_ATTRIB_BITS 4294967295

#define GL_CLIENT_ALL_ATTRIB_BITS 4294967295

#define GL_CURRENT_BIT 1

#define GL_POINT_BIT 2

#define GL_LINE_BIT 4

#define GL_POLYGON_BIT 8

#define GL_POLYGON_STIPPLE_BIT 16

#define GL_PIXEL_MODE_BIT 32

#define GL_LIGHTING_BIT 64

#define GL_FOG_BIT 128

#define GL_DEPTH_BUFFER_BIT_ATTRIB 256

#define GL_ACCUM_BUFFER_BIT 512

#define GL_STENCIL_BUFFER_BIT_ATTRIB 1024

#define GL_VIEWPORT_BIT 2048

#define GL_TRANSFORM_BIT 4096

#define GL_ENABLE_BIT 8192

#define GL_COLOR_BUFFER_BIT_ATTRIB 16384

#define GL_HINT_BIT 32768

#define GL_EVAL_BIT 65536

#define GL_LIST_BIT 131072

#define GL_TEXTURE_BIT 262144

#define GL_SCISSOR_BIT 524288

#define GL_CLIENT_PIXEL_STORE_BIT 1

#define GL_CLIENT_VERTEX_ARRAY_BIT 2

#define GL_MODULATE 8448

#define GL_DECAL 8449

#define GL_ADD 260

#define GL_TEXTURE_ENV 8960

#define GL_TEXTURE_ENV_MODE 8704

#define GL_TEXTURE_ENV_COLOR 8705

#define GL_COMBINE 34160

#define GL_SRC0_RGB 34176

#define GL_SRC1_RGB 34177

#define GL_SRC2_RGB 34178

#define GL_OPERAND0_RGB 34192

#define GL_OPERAND1_RGB 34193

#define GL_OPERAND2_RGB 34194

#define GL_RGB_SCALE 34163

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

void glMatrixMode(GLenum mode);

void glLoadIdentity(void);

void glPushMatrix(void);

void glPopMatrix(void);

void glTranslatef(GLfloat x, GLfloat y, GLfloat z);

void glTranslated(GLdouble x, GLdouble y, GLdouble z);

void glRotatef(GLfloat angle, GLfloat x, GLfloat y, GLfloat z);

void glScalef(GLfloat x, GLfloat y, GLfloat z);

void glScaled(GLdouble x, GLdouble y, GLdouble z);

void glOrtho(GLdouble left,
             GLdouble right,
             GLdouble bottom,
             GLdouble top,
             GLdouble near_val,
             GLdouble far_val);

void glOrthof(GLfloat left,
              GLfloat right,
              GLfloat bottom,
              GLfloat top,
              GLfloat near_val,
              GLfloat far_val);

void glFrustum(GLdouble left,
               GLdouble right,
               GLdouble bottom,
               GLdouble top,
               GLdouble near_val,
               GLdouble far_val);

void glFrustumf(GLfloat left,
                GLfloat right,
                GLfloat bottom,
                GLfloat top,
                GLfloat near_val,
                GLfloat far_val);

void glMultMatrixf(const GLfloat *m);

void glLoadMatrixf(const GLfloat *m);

void glEnableClientState(GLenum array);

void glDisableClientState(GLenum array);

void glVertexPointer(GLint size, GLenum type_, GLsizei stride, const void *pointer);

void glTexCoordPointer(GLint size, GLenum type_, GLsizei stride, const void *pointer);

void glColorPointer(GLint size, GLenum type_, GLsizei stride, const void *pointer);

void glNormalPointer(GLenum type_, GLsizei stride, const void *pointer);

void glClientActiveTexture(GLenum texture);

void glDrawArrays(GLenum mode, GLint first, GLsizei count);

void glDrawElements(GLenum mode, GLsizei count, GLenum type_, const void *indices);

void glBegin(GLenum mode);

void glEnd(void);

void glVertex3f(GLfloat x, GLfloat y, GLfloat z);

void glVertex2f(GLfloat x, GLfloat y);

void glTexCoord2f(GLfloat u, GLfloat v);

void glMultiTexCoord2f(GLenum _target, GLfloat s, GLfloat t);

void glMultiTexCoord4f(GLenum _target, GLfloat s, GLfloat t, GLfloat _r, GLfloat _q);

void glColor4f(GLfloat r, GLfloat g, GLfloat b, GLfloat a);

void glColor3f(GLfloat r, GLfloat g, GLfloat b);

void glColor4ub(GLubyte r, GLubyte g, GLubyte b, GLubyte a);

void glColor3ub(GLubyte r, GLubyte g, GLubyte b);

void glColor4fv(const GLfloat *v);

void glNormal3f(GLfloat x, GLfloat y, GLfloat z);

GLuint glGenLists(GLsizei range);

void glDeleteLists(GLuint list, GLsizei range);

void glNewList(GLuint list, GLenum mode);

void glEndList(void);

void glCallList(GLuint list);

void glCallLists(GLsizei n, GLenum type_, const void *lists);

GLboolean glIsList(GLuint list);

void glGenTextures(GLsizei n, GLuint *textures);

void glDeleteTextures(GLsizei n, const GLuint *textures);

void glBindTexture(GLenum target, GLuint texture);

void glTexImage2D(GLenum _target,
                  GLint level,
                  GLint internalformat,
                  GLsizei width,
                  GLsizei height,
                  GLint _border,
                  GLenum format,
                  GLenum type_,
                  const void *pixels);

void glTexSubImage2D(GLenum _target,
                     GLint level,
                     GLint xoffset,
                     GLint yoffset,
                     GLsizei width,
                     GLsizei height,
                     GLenum format,
                     GLenum type_,
                     const void *pixels);

void glTexParameteri(GLenum _target, GLenum pname, GLint param);

void glTexParameterf(GLenum target, GLenum pname, GLfloat param);

void glTexParameteriv(GLenum target, GLenum pname, const GLint *params);

void glTexParameterfv(GLenum target, GLenum pname, const GLfloat *params);

void glActiveTexture(GLenum texture);

void glTexImage1D(GLenum target,
                  GLint level,
                  GLint internalformat,
                  GLsizei width,
                  GLint border,
                  GLenum format,
                  GLenum type_,
                  const void *pixels);

void glTexImage3D(GLenum _target,
                  GLint _level,
                  GLint _internalformat,
                  GLsizei _width,
                  GLsizei _height,
                  GLsizei _depth,
                  GLint _border,
                  GLenum _format,
                  GLenum _type_,
                  const void *_pixels);

void glGetTexLevelParameteri(GLenum _target, GLint _level, GLenum pname, GLint *params);

void glTexGen(GLenum _coord, GLenum _pname, GLfloat _param);

void glTexGeni(GLenum _coord, GLenum _pname, GLint _param);

void glTexEnvf(GLenum _target, GLenum _pname, GLfloat _param);

void glTexEnvi(GLenum _target, GLenum _pname, GLint _param);

void glTexEnvfv(GLenum _target, GLenum _pname, const GLfloat *_params);

void glEnable(GLenum cap);

void glDisable(GLenum cap);

GLboolean glIsEnabled(GLenum cap);

void glAlphaFunc(GLenum func, GLclampf ref_val);

void glBlendFunc(GLenum sfactor, GLenum dfactor);

void glBlendColor(GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha);

void glBlendFuncSeparate(GLenum srcRGB, GLenum dstRGB, GLenum srcAlpha, GLenum dstAlpha);

void glDepthFunc(GLenum func);

void glDepthMask(GLboolean flag);

void glColorMask(GLboolean red, GLboolean green, GLboolean blue, GLboolean alpha);

void glCullFace(GLenum mode);

void glFrontFace(GLenum mode);

void glPolygonOffset(GLfloat factor, GLfloat units);

void glLineWidth(GLfloat width);

void glPointSize(GLfloat size);

void glShadeModel(GLenum mode);

void glColorMaterial(GLenum _face, GLenum _mode);

void glFogf(GLenum pname, GLfloat param);

void glFogfv(GLenum pname, const GLfloat *params);

void glFogi(GLenum pname, GLint param);

void glFogx(GLenum pname, GLfixed param);

void glFogxv(GLenum pname, const GLfixed *params);

void glHint(GLenum target, GLenum mode);

void glDepthRangef(GLclampf n, GLclampf f);

void glDepthRange(GLclampd n, GLclampd f);

void glDepthRangex(GLclampx n, GLclampx f);

void glLightf(GLenum _light, GLenum _pname, GLfloat _param);

void glLightfv(GLenum light, GLenum pname, const GLfloat *params);

void glLightModelf(GLenum _pname, GLfloat _param);

void glLightModelfv(GLenum pname, const GLfloat *params);

void glMaterialf(GLenum _face, GLenum _pname, GLfloat _param);

void glMaterialfv(GLenum _face, GLenum _pname, const GLfloat *_params);

void glStencilFunc(GLenum func, GLint ref_val, GLuint mask);

void glStencilMask(GLuint mask);

void glStencilOp(GLenum fail, GLenum zfail, GLenum zpass);

void glViewport(GLint x, GLint y, GLsizei width, GLsizei height);

void glScissor(GLint x, GLint y, GLsizei width, GLsizei height);

void glClearColor(GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha);

void glClearDepthf(GLclampf depth);

void glClearDepth(GLclampd depth);

void glClearStencil(GLint s);

void glClear(GLbitfield mask);

void glPixelStorei(GLenum _pname, GLint _param);

void glReadPixels(GLint _x,
                  GLint _y,
                  GLsizei width,
                  GLsizei height,
                  GLenum _format,
                  GLenum _type_,
                  void *pixels);

void glFlush(void);

void glFinish(void);

void glGetIntegerv(GLenum pname, GLint *params);

void glGetFloatv(GLenum pname, GLfloat *params);

void glGetBooleanv(GLenum pname, GLboolean *params);

const GLubyte *glGetString(GLenum name);

GLenum glGetError(void);

void glPushAttrib(GLbitfield _mask);

void glPopAttrib(void);

void glPushClientAttrib(GLbitfield _mask);

void glPopClientAttrib(void);

GLuint glCreateShader(GLenum shader_type);

void glShaderSource(GLuint _shader,
                    GLsizei _count,
                    const GLchar *const *_string,
                    const GLint *_length);

void glCompileShader(GLuint _shader);

void glGetShaderiv(GLuint _shader, GLenum pname, GLint *params);

void glGetShaderInfoLog(GLuint _shader, GLsizei _buf_size, GLsizei *length, GLchar *info_log);

void glDeleteShader(GLuint _shader);

GLuint glCreateProgram(void);

void glAttachShader(GLuint _program, GLuint _shader);

void glDetachShader(GLuint _program, GLuint _shader);

void glLinkProgram(GLuint _program);

void glGetProgramiv(GLuint _program, GLenum pname, GLint *params);

void glGetProgramInfoLog(GLuint _program, GLsizei _buf_size, GLsizei *length, GLchar *info_log);

void glUseProgram(GLuint _program);

void glDeleteProgram(GLuint _program);

GLint glGetUniformLocation(GLuint _program, const GLchar *_name);

GLint glGetAttribLocation(GLuint _program, const GLchar *_name);

void glUniform1f(GLint _location, GLfloat _v0);

void glUniform2f(GLint _location, GLfloat _v0, GLfloat _v1);

void glUniform3f(GLint _location, GLfloat _v0, GLfloat _v1, GLfloat _v2);

void glUniform4f(GLint _location, GLfloat _v0, GLfloat _v1, GLfloat _v2, GLfloat _v3);

void glUniform1i(GLint _location, GLint _v0);

void glUniform2i(GLint _location, GLint _v0, GLint _v1);

void glUniform3i(GLint _location, GLint _v0, GLint _v1, GLint _v2);

void glUniform4i(GLint _location, GLint _v0, GLint _v1, GLint _v2, GLint _v3);

void glUniformMatrix4fv(GLint _location,
                        GLsizei _count,
                        GLboolean _transpose,
                        const GLfloat *_value);

void glGenBuffers(GLsizei n, GLuint *buffers);

void glGenBuffersARB(GLsizei n, GLuint *buffers);

void glBindBuffer(GLenum _target, GLuint _buffer);

void glBindBufferARB(GLenum target, GLuint buffer);

void glBufferData(GLenum _target, GLsizeiptr _size, const void *_data, GLenum _usage);

void glBufferDataARB(GLenum target, GLsizeiptr size, const void *data, GLenum usage);

void glBufferSubData(GLenum _target, GLintptr _offset, GLsizeiptr _size, const void *_data);

void glDeleteBuffers(GLsizei _n, const GLuint *_buffers);

void glDeleteBuffersARB(GLsizei n, const GLuint *buffers);

void glVertexAttribPointer(GLuint _index,
                           GLint _size,
                           GLenum _type_,
                           GLboolean _normalized,
                           GLsizei _stride,
                           const void *_pointer);

void glEnableVertexAttribArray(GLuint _index);

void glDisableVertexAttribArray(GLuint _index);

void glGenFramebuffers(GLsizei n, GLuint *framebuffers);

void glBindFramebuffer(GLenum _target, GLuint _framebuffer);

void glFramebufferTexture2D(GLenum _target,
                            GLenum _attachment,
                            GLenum _textarget,
                            GLuint _texture,
                            GLint _level);

void glDeleteFramebuffers(GLsizei _n, const GLuint *_framebuffers);

GLenum glCheckFramebufferStatus(GLenum _target);

void glGenRenderbuffers(GLsizei n, GLuint *renderbuffers);

void glBindRenderbuffer(GLenum _target, GLuint _renderbuffer);

void glRenderbufferStorage(GLenum _target, GLenum _internalformat, GLsizei _width, GLsizei _height);

void glDeleteRenderbuffers(GLsizei _n, const GLuint *_renderbuffers);

void glGenerateMipmap(GLenum _target);

void glGenQueries(GLsizei n, GLuint *ids);

void glGenQueriesARB(GLsizei n, GLuint *ids);

void glBeginQuery(GLenum _target, GLuint _id);

void glBeginQueryARB(GLenum target, GLuint id);

void glEndQuery(GLenum _target);

void glEndQueryARB(GLenum target);

void glGetQueryObjectuiv(GLuint _id, GLenum _pname, GLuint *params);

void glGetQueryObjectuivARB(GLuint id, GLenum pname, GLuint *params);

void glDeleteQueries(GLsizei _n, const GLuint *_ids);

void glDeleteQueriesARB(GLsizei n, const GLuint *ids);

EGLDisplay eglGetDisplay(NativeDisplayType display_id);

EGLBoolean eglInitialize(EGLDisplay dpy, EGLint *major, EGLint *minor);

EGLBoolean eglTerminate(EGLDisplay dpy);

EGLBoolean eglGetConfigs(EGLDisplay dpy,
                         EGLConfig *configs,
                         EGLint config_size,
                         EGLint *num_config);

EGLBoolean eglChooseConfig(EGLDisplay dpy,
                           const EGLint *attrib_list,
                           EGLConfig *configs,
                           EGLint config_size,
                           EGLint *num_config);

EGLBoolean eglGetConfigAttrib(EGLDisplay dpy, EGLConfig config, EGLint attribute, EGLint *value);

EGLSurface eglCreateWindowSurface(EGLDisplay dpy,
                                  EGLConfig config,
                                  NativeWindowType win,
                                  const EGLint *attrib_list);

EGLSurface angle_wgpu_create_native_window_surface(EGLDisplay dpy,
                                                   EGLConfig _config,
                                                   const struct AngleWgpuNativeWindow *native);

EGLBoolean angle_wgpu_resize_surface(EGLSurface surface, uint32_t width, uint32_t height);

EGLBoolean eglBindAPI(EGLenum _api);

EGLSurface eglCreatePbufferSurface(EGLDisplay dpy, EGLConfig config, const EGLint *attrib_list);

EGLBoolean eglDestroySurface(EGLDisplay dpy, EGLSurface surface);

EGLContext eglCreateContext(EGLDisplay dpy,
                            EGLConfig config,
                            EGLContext share_context,
                            const EGLint *attrib_list);

EGLBoolean eglDestroyContext(EGLDisplay dpy, EGLContext ctx);

EGLBoolean eglMakeCurrent(EGLDisplay dpy, EGLSurface draw, EGLSurface read, EGLContext ctx);

EGLContext eglGetCurrentContext(void);

EGLSurface eglGetCurrentSurface(EGLint readdraw);

EGLDisplay eglGetCurrentDisplay(void);

EGLBoolean eglQuerySurface(EGLDisplay dpy, EGLSurface surface, EGLint attribute, EGLint *value);

EGLBoolean eglSwapBuffers(EGLDisplay dpy, EGLSurface surface);

EGLBoolean eglSwapInterval(EGLDisplay dpy, EGLint interval);

EGLint eglGetError(void);

__eglMustCastToProperFunctionPointerType eglGetProcAddress(const char *procname);

bool angle_wgpu_decode_png_memory(const uint8_t *data, size_t len, struct DecodedImage *out_image);

bool angle_wgpu_decode_png_file(const char *path, struct DecodedImage *out_image);

void angle_wgpu_free_decoded_image(struct DecodedImage *image);

struct WinitApp *winit_app_create(const char *title,
                                  uint32_t width,
                                  uint32_t height,
                                  bool resizable);

void winit_app_destroy(struct WinitApp *app);

bool winit_app_pump_events(struct WinitApp *app);

void winit_app_poll_events(struct WinitApp *app);

bool winit_app_should_close(struct WinitApp *app);

void winit_app_get_size(struct WinitApp *app, uint32_t *width, uint32_t *height);

void winit_app_set_mouse_grab(struct WinitApp *app, bool grab);

void winit_app_set_cursor_visible(struct WinitApp *app, bool visible);

bool winit_app_is_key_down(struct WinitApp *app, uint32_t keycode);

bool winit_app_is_button_down(struct WinitApp *app, uint32_t button);

void winit_app_get_mouse_pos(struct WinitApp *app, double *x, double *y);

void winit_app_get_mouse_delta(struct WinitApp *app, double *dx, double *dy);

float winit_app_consume_wheel_delta(struct WinitApp *app);

EGLSurface winit_app_create_egl_surface(struct WinitApp *app, EGLDisplay dpy, EGLConfig _config);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* ANGLE_WGPU_H */
