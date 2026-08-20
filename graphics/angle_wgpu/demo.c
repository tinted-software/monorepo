#include "angle_wgpu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <math.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
#define sleep_ms(ms) Sleep(ms)
#else
#include <unistd.h>
#define sleep_ms(ms) usleep((ms) * 1000)
#endif

int main(int argc, char **argv) {
    bool windowed_mode = true;
    int max_frames = 0; // 0 = run indefinitely until window close or Esc

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--headless") == 0 || strcmp(argv[i], "-h") == 0) {
            windowed_mode = false;
        } else if (strcmp(argv[i], "--frames") == 0 && i + 1 < argc) {
            max_frames = atoi(argv[++i]);
        }
    }

    printf("========================================\n");
    printf(" angle_wgpu C Windowed Demo Program     \n");
    printf(" Mode: %s\n", windowed_mode ? "Interactive Windowed" : "Headless Offscreen");
    printf("========================================\n\n");

    // 1. Initialize EGL
    printf("[1] Initializing EGL display...\n");
    EGLDisplay dpy = eglGetDisplay(NULL);
    if (!dpy) {
        fprintf(stderr, "Failed to get EGL display\n");
        return 1;
    }

    EGLint major = 0, minor = 0;
    if (!eglInitialize(dpy, &major, &minor)) {
        fprintf(stderr, "Failed to initialize EGL\n");
        return 1;
    }
    printf("    EGL version: %d.%d\n", major, minor);

    // 2. Choose Config
    printf("[2] Choosing EGL config...\n");
    EGLConfig config = NULL;
    EGLint num_configs = 0;
    EGLint config_attribs[] = { EGL_NONE };
    if (!eglChooseConfig(dpy, config_attribs, &config, 1, &num_configs) || num_configs < 1) {
        fprintf(stderr, "Failed to choose EGL config\n");
        return 1;
    }

    // 3. Create Surface (Windowed or PBuffer)
    WinitApp *app = NULL;
    EGLSurface surf = NULL;
    uint32_t width = 800;
    uint32_t height = 600;

    if (windowed_mode) {
        printf("[3] Opening Winit Window (800x600)...\n");
        app = winit_app_create("angle_wgpu Demo Window", width, height, true);
        if (!app) {
            fprintf(stderr, "Failed to create Winit window, falling back to offscreen\n");
            windowed_mode = false;
        } else {
            surf = winit_app_create_egl_surface(app, dpy, config);
            if (!surf) {
                fprintf(stderr, "Failed to create EGL surface for window, falling back to offscreen\n");
                winit_app_destroy(app);
                app = NULL;
                windowed_mode = false;
            }
        }
    }

    if (!windowed_mode) {
        printf("[3] Creating EGL PBuffer Surface (800x600)...\n");
        EGLint pbuffer_attribs[] = {
            EGL_WIDTH, 800,
            EGL_HEIGHT, 600,
            EGL_NONE
        };
        surf = eglCreatePbufferSurface(dpy, config, pbuffer_attribs);
    }

    if (!surf) {
        fprintf(stderr, "Fatal: Could not create any EGL surface\n");
        return 1;
    }

    // 4. Create Context and Make Current
    printf("[4] Creating EGL Context and binding to surface...\n");
    EGLContext ctx = eglCreateContext(dpy, config, NULL, NULL);
    if (!ctx) {
        fprintf(stderr, "Failed to create EGL context\n");
        return 1;
    }

    if (!eglMakeCurrent(dpy, surf, surf, ctx)) {
        fprintf(stderr, "Failed to make EGL context current\n");
        return 1;
    }

    // 5. Query GL Info
    const GLubyte *vendor = glGetString(GL_VENDOR);
    const GLubyte *renderer = glGetString(GL_RENDERER);
    const GLubyte *version = glGetString(GL_VERSION);
    printf("    GL Vendor:   %s\n", vendor ? (const char*)vendor : "null");
    printf("    GL Renderer: %s\n", renderer ? (const char*)renderer : "null");
    printf("    GL Version:  %s\n\n", version ? (const char*)version : "null");

    // 6. Create Texture (Checkerboard pattern)
    printf("[5] Uploading texture...\n");
    GLuint tex = 0;
    glGenTextures(1, &tex);
    glBindTexture(GL_TEXTURE_2D, tex);

    #define PAT_W 8
    #define PAT_H 8
    uint32_t pattern_pixels[PAT_W * PAT_H];
    for (int y = 0; y < PAT_H; ++y) {
        for (int x = 0; x < PAT_W; ++x) {
            bool check = ((x + y) % 2) == 0;
            pattern_pixels[y * PAT_W + x] = check ? 0xFF30D5C8 : 0xFFFF4500; // Turquoise & OrangeRed
        }
    }
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, PAT_W, PAT_H, 0, GL_RGBA, GL_UNSIGNED_BYTE, pattern_pixels);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glEnable(GL_TEXTURE_2D);

    // 7. Compile 3D Cube into Display List
    printf("[6] Compiling 3D Rotating Cube Display List...\n");
    GLuint cube_list = glGenLists(1);
    glNewList(cube_list, GL_COMPILE);
    glBegin(GL_QUADS);

    // Front face
    glTexCoord2f(0.0f, 0.0f); glColor4f(1.0f, 0.2f, 0.2f, 1.0f); glVertex3f(-1.0f, -1.0f,  1.0f);
    glTexCoord2f(1.0f, 0.0f); glColor4f(1.0f, 0.2f, 0.2f, 1.0f); glVertex3f( 1.0f, -1.0f,  1.0f);
    glTexCoord2f(1.0f, 1.0f); glColor4f(1.0f, 0.5f, 0.5f, 1.0f); glVertex3f( 1.0f,  1.0f,  1.0f);
    glTexCoord2f(0.0f, 1.0f); glColor4f(1.0f, 0.5f, 0.5f, 1.0f); glVertex3f(-1.0f,  1.0f,  1.0f);

    // Back face
    glTexCoord2f(1.0f, 0.0f); glColor4f(0.2f, 1.0f, 0.2f, 1.0f); glVertex3f(-1.0f, -1.0f, -1.0f);
    glTexCoord2f(1.0f, 1.0f); glColor4f(0.5f, 1.0f, 0.5f, 1.0f); glVertex3f(-1.0f,  1.0f, -1.0f);
    glTexCoord2f(0.0f, 1.0f); glColor4f(0.2f, 1.0f, 0.2f, 1.0f); glVertex3f( 1.0f,  1.0f, -1.0f);
    glTexCoord2f(0.0f, 0.0f); glColor4f(0.5f, 1.0f, 0.5f, 1.0f); glVertex3f( 1.0f, -1.0f, -1.0f);

    // Top face
    glTexCoord2f(0.0f, 1.0f); glColor4f(0.2f, 0.2f, 1.0f, 1.0f); glVertex3f(-1.0f,  1.0f, -1.0f);
    glTexCoord2f(0.0f, 0.0f); glColor4f(0.2f, 0.2f, 1.0f, 1.0f); glVertex3f(-1.0f,  1.0f,  1.0f);
    glTexCoord2f(1.0f, 0.0f); glColor4f(0.5f, 0.5f, 1.0f, 1.0f); glVertex3f( 1.0f,  1.0f,  1.0f);
    glTexCoord2f(1.0f, 1.0f); glColor4f(0.5f, 0.5f, 1.0f, 1.0f); glVertex3f( 1.0f,  1.0f, -1.0f);

    // Bottom face
    glTexCoord2f(1.0f, 1.0f); glColor4f(1.0f, 1.0f, 0.2f, 1.0f); glVertex3f(-1.0f, -1.0f, -1.0f);
    glTexCoord2f(0.0f, 1.0f); glColor4f(1.0f, 1.0f, 0.2f, 1.0f); glVertex3f( 1.0f, -1.0f, -1.0f);
    glTexCoord2f(0.0f, 0.0f); glColor4f(1.0f, 1.0f, 0.5f, 1.0f); glVertex3f( 1.0f, -1.0f,  1.0f);
    glTexCoord2f(1.0f, 0.0f); glColor4f(1.0f, 1.0f, 0.5f, 1.0f); glVertex3f(-1.0f, -1.0f,  1.0f);

    // Right face
    glTexCoord2f(1.0f, 0.0f); glColor4f(1.0f, 0.2f, 1.0f, 1.0f); glVertex3f( 1.0f, -1.0f, -1.0f);
    glTexCoord2f(1.0f, 1.0f); glColor4f(1.0f, 0.5f, 1.0f, 1.0f); glVertex3f( 1.0f,  1.0f, -1.0f);
    glTexCoord2f(0.0f, 1.0f); glColor4f(1.0f, 0.5f, 1.0f, 1.0f); glVertex3f( 1.0f,  1.0f,  1.0f);
    glTexCoord2f(0.0f, 0.0f); glColor4f(1.0f, 0.2f, 1.0f, 1.0f); glVertex3f( 1.0f, -1.0f,  1.0f);

    // Left face
    glTexCoord2f(0.0f, 0.0f); glColor4f(0.2f, 1.0f, 1.0f, 1.0f); glVertex3f(-1.0f, -1.0f, -1.0f);
    glTexCoord2f(1.0f, 0.0f); glColor4f(0.2f, 1.0f, 1.0f, 1.0f); glVertex3f(-1.0f, -1.0f,  1.0f);
    glTexCoord2f(1.0f, 1.0f); glColor4f(0.5f, 1.0f, 1.0f, 1.0f); glVertex3f(-1.0f,  1.0f,  1.0f);
    glTexCoord2f(0.0f, 1.0f); glColor4f(0.5f, 1.0f, 1.0f, 1.0f); glVertex3f(-1.0f,  1.0f, -1.0f);

    glEnd();
    glEndList();

    // 8. Main Render & Event Loop
    printf("[7] Starting render loop (Press ESC or Close Window to exit)...\n");
    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LEQUAL);

    float rot_x = 0.0f;
    float rot_y = 0.0f;
    float rot_z = 0.0f;
    int frame_idx = 0;

    while (true) {
        if (app) {
            winit_app_poll_events(app);
            if (winit_app_should_close(app)) {
                printf("Window close requested.\n");
                break;
            }
            uint32_t cur_w = width, cur_h = height;
            winit_app_get_size(app, &cur_w, &cur_h);
            if (cur_w > 0 && cur_h > 0) {
                width = cur_w;
                height = cur_h;
            }
        }

        // Viewport & Projection setup
        glViewport(0, 0, width, height);
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();

        float aspect = (float)width / (float)height;
        // Simple perspective-like matrix or orthographic
        glOrthof(-3.0f * aspect, 3.0f * aspect, -3.0f, 3.0f, -20.0f, 20.0f);

        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();
        glTranslatef(0.0f, 0.0f, -5.0f);
        glRotatef(rot_x, 1.0f, 0.0f, 0.0f);
        glRotatef(rot_y, 0.0f, 1.0f, 0.0f);
        glRotatef(rot_z, 0.0f, 0.0f, 1.0f);

        // Clear Color & Depth
        glClearColor(0.12f, 0.15f, 0.22f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        // Render Cube
        glCallList(cube_list);

        // Present Frame
        eglSwapBuffers(dpy, surf);

        rot_x += 0.8f;
        rot_y += 1.2f;
        rot_z += 0.5f;
        frame_idx++;

        if (max_frames > 0 && frame_idx >= max_frames) {
            printf("Reached target frame limit (%d frames).\n", max_frames);
            break;
        }

        // Limit frame rate slightly (~60fps)
        sleep_ms(16);
    }

    // 9. Teardown Resources
    printf("\n[8] Cleaning up resources...\n");
    glDeleteLists(cube_list, 1);
    glDeleteTextures(1, &tex);

    eglMakeCurrent(dpy, NULL, NULL, NULL);
    eglDestroyContext(dpy, ctx);
    eglDestroySurface(dpy, surf);
    eglTerminate(dpy);

    if (app) {
        winit_app_destroy(app);
    }

    printf("Demo finished successfully.\n");
    return 0;
}
