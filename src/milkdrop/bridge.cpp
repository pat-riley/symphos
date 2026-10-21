// Symphos host bridge. Each handle and its OpenGL context stay on one worker thread.
#include "ProjectM.hpp"
#include "Logging.hpp"
#include "glad/gl.h"
#include "Renderer/Platform/GLResolver.hpp"
#include "Renderer/Platform/GladLoader.hpp"
#include <algorithm>
#include <cstring>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <vector>
#ifdef __APPLE__
#include <OpenGL/OpenGL.h>
#else
#include <EGL/egl.h>
#include <EGL/eglext.h>
#endif

struct Engine : libprojectM::ProjectM {
    mutable std::string error;
    void PresetSwitchFailedEvent(const std::string&, const std::string& message) const override { error = message; }
};
struct Host {
#ifdef __APPLE__
    CGLContextObj context{};
    Host() {
        CGLPixelFormatAttribute attrs[] = {kCGLPFAOpenGLProfile, (CGLPixelFormatAttribute)kCGLOGLPVersion_3_2_Core, kCGLPFAAccelerated, (CGLPixelFormatAttribute)0};
        CGLPixelFormatObj format{}; GLint count{};
        if (CGLChoosePixelFormat(attrs, &format, &count) != kCGLNoError || !format) throw std::runtime_error("Cannot create an accelerated OpenGL pixel format");
        auto result = CGLCreateContext(format, nullptr, &context); CGLDestroyPixelFormat(format);
        if (result != kCGLNoError) throw std::runtime_error("Cannot create the MilkDrop OpenGL context");
        CGLSetCurrentContext(context);
    }
    ~Host() { if(context) { CGLSetCurrentContext(nullptr); CGLDestroyContext(context); } }
#else
    EGLDisplay display{EGL_NO_DISPLAY}; EGLContext context{EGL_NO_CONTEXT}; EGLSurface surface{EGL_NO_SURFACE};
    Host() {
        display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
        if (!eglInitialize(display, nullptr, nullptr)) {
            display = eglGetPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA, EGL_DEFAULT_DISPLAY, nullptr);
            if (!eglInitialize(display, nullptr, nullptr)) throw std::runtime_error("Cannot initialize EGL for MilkDrop");
        }
        eglBindAPI(EGL_OPENGL_API);
        const EGLint configAttrs[] = {EGL_SURFACE_TYPE,EGL_PBUFFER_BIT,EGL_RENDERABLE_TYPE,EGL_OPENGL_BIT,EGL_NONE};
        EGLConfig config{}; EGLint count{};
        eglChooseConfig(display, configAttrs, &config, 1, &count);
        const EGLint attrs[] = {EGL_CONTEXT_MAJOR_VERSION,3,EGL_CONTEXT_MINOR_VERSION,3,EGL_CONTEXT_OPENGL_PROFILE_MASK,EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,EGL_NONE};
        context = eglCreateContext(display, config, EGL_NO_CONTEXT, attrs);
        const EGLint surfaceAttrs[] = {EGL_WIDTH,1,EGL_HEIGHT,1,EGL_NONE};
        surface = eglCreatePbufferSurface(display, config, surfaceAttrs);
        if(context == EGL_NO_CONTEXT || surface == EGL_NO_SURFACE || !eglMakeCurrent(display,surface,surface,context)) throw std::runtime_error("Cannot create MilkDrop OpenGL 3.3 context");
    }
    ~Host() { if(display != EGL_NO_DISPLAY) { eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,EGL_NO_CONTEXT); if(surface != EGL_NO_SURFACE) eglDestroySurface(display,surface); if(context != EGL_NO_CONTEXT) eglDestroyContext(display,context); /* EGLDisplay can be shared with other component workers. */ } }
#endif
};
struct Instance {
    Host host; // Must outlive GL objects and engine.
    std::unique_ptr<Engine> engine;
    GLuint fbo{}, texture{};
    unsigned width{},height{};
    std::string warning;
    Instance() {
        if (!libprojectM::Renderer::Platform::GLResolver::Instance().Initialize(nullptr, nullptr) || !libprojectM::Renderer::Platform::GladLoader::Instance().Initialize()) throw std::runtime_error("Cannot initialize OpenGL function loader");
        engine = std::make_unique<Engine>();
        engine->SetTargetFramesPerSecond(60);
        engine->SetPresetLocked(true);
        engine->SetHardCutEnabled(false);
        glGenFramebuffers(1, &fbo); glGenTextures(1, &texture);
    }
    ~Instance() { libprojectM::Logging::SetThreadCallback({}); glDeleteFramebuffers(1,&fbo); glDeleteTextures(1,&texture); }
    void resize(unsigned w,unsigned h) {
        if(width==w && height==h) return;
        width=w;height=h;
        glBindTexture(GL_TEXTURE_2D,texture);
        glTexImage2D(GL_TEXTURE_2D,0,GL_RGBA8,w,h,0,GL_RGBA,GL_UNSIGNED_BYTE,nullptr);
        glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MIN_FILTER,GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MAG_FILTER,GL_LINEAR);
        glBindFramebuffer(GL_FRAMEBUFFER,fbo);
        glFramebufferTexture2D(GL_FRAMEBUFFER,GL_COLOR_ATTACHMENT0,GL_TEXTURE_2D,texture,0);
        if(glCheckFramebufferStatus(GL_FRAMEBUFFER)!=GL_FRAMEBUFFER_COMPLETE) throw std::runtime_error("MilkDrop framebuffer incomplete");
        engine->SetWindowSize(w,h);
    }
};
static void message(char* out, size_t size, const std::string& s) { if(size) { std::strncpy(out,s.c_str(),size-1);out[size-1]=0; } }
extern "C" {
void* symphos_milk_create(char* error,size_t size) noexcept {
    try { return new Instance(); } catch(const std::exception& e) { message(error,size,e.what()); return nullptr; } catch(...) { message(error,size,"Unknown renderer initialization error");return nullptr; }
}
void symphos_milk_destroy(void* ptr) noexcept { delete static_cast<Instance*>(ptr); }
int symphos_milk_load(void* ptr,const char* source,const char* textures,int smooth,float seconds,char* error,size_t size) noexcept {
    try {
        auto& p=*static_cast<Instance*>(ptr);
        p.resize(p.width ? p.width : 640,p.height ? p.height : 360);
        p.engine->error.clear();
        p.warning.clear();
        libprojectM::Logging::SetThreadLogLevel(libprojectM::Logging::LogLevel::Warning);
        libprojectM::Logging::SetThreadCallback({[](const char* text, int, void* data) {
            auto& warning = static_cast<Instance*>(data)->warning;
            if (warning.size() < 3000 && warning.find(text) == std::string::npos) { warning += text; warning += "\n"; }
        }, &p});
        p.engine->SetSoftCutDuration(seconds);
        p.engine->SetPresetStartClean(!smooth);
        p.engine->SetTexturePaths({textures});
        std::istringstream data(source);
        p.engine->LoadPresetData(data,smooth!=0);
        if(!p.engine->error.empty()) throw std::runtime_error(p.engine->error);
        auto code=glGetError(); if(code!=GL_NO_ERROR) throw std::runtime_error("OpenGL preset load error: "+std::to_string(code));
        return 1;
    } catch(const std::exception& e) {message(error,size,e.what());return 0;} catch(...) {message(error,size,"Unknown preset load error");return 0;}
}
void symphos_milk_warnings(void* ptr, char* out, size_t size) noexcept {
    message(out, size, static_cast<Instance*>(ptr)->warning);
}
int symphos_milk_render(void* ptr,unsigned w,unsigned h,double time,const float* pcm,size_t count,const float* controls,float sensitivity,unsigned mesh,unsigned char* rgba,char* error,size_t size) noexcept {
    try {
        auto& p=*static_cast<Instance*>(ptr);
        p.resize(w,h);
        std::copy(controls,controls+32,p.engine->symphosControls.begin());
        p.engine->SetBeatSensitivity(sensitivity);
        p.engine->SetMeshSize(mesh,mesh*3/4);
        p.engine->SetFrameTime(time);
        if(count) p.engine->PCM().Add(pcm,2,count);
        p.engine->RenderFrame(p.fbo);
        glBindFramebuffer(GL_FRAMEBUFFER,p.fbo);
        glPixelStorei(GL_PACK_ALIGNMENT,1);
        glReadPixels(0,0,w,h,GL_RGBA,GL_UNSIGNED_BYTE,rgba);
        for(size_t i=3;i<static_cast<size_t>(w)*h*4;i+=4) rgba[i]=255;
        auto code=glGetError();
        if(code!=GL_NO_ERROR) throw std::runtime_error("OpenGL render error: "+std::to_string(code));
        return 1;
    } catch(const std::exception& e) {message(error,size,e.what());return 0;} catch(...) {message(error,size,"Unknown renderer error");return 0;}
}
}
