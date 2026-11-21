// FFmpeg owns vendor-specific encoding and resource lifetimes. Kokoda passes
// NV12 frames through its public API; no direct NVENC/AMF/QSV SDK calls.
#define NOMINMAX
#include <windows.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <algorithm>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>
}
using Microsoft::WRL::ComPtr;
static void error_text(char* out,size_t cap,const char* text) noexcept {
    if(out && cap) {size_t n=std::min(cap-1,std::strlen(text));std::memcpy(out,text,n);out[n]=0;}
}
struct Library {
    HMODULE handle=nullptr;
    explicit Library(const wchar_t* name) {
        wchar_t path[32768];DWORD n=GetModuleFileNameW(nullptr,path,32768);
        if(!n || n>=32768) throw std::runtime_error("Cannot locate executable directory");
        std::wstring file(path,n);file.resize(file.find_last_of(L"\\/")+1);
        file+=L"ffmpeg\\";file+=name;
        handle=LoadLibraryExW(file.c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_SYSTEM32);
        if(!handle) throw std::runtime_error("Cannot load bundled FFmpeg DLL, Win32="+std::to_string(GetLastError()));
    }
    ~Library(){if(handle)FreeLibrary(handle);}
    template<class T> T get(const char* name) {
        auto p=GetProcAddress(handle,name);
        if(!p) throw std::runtime_error(std::string("Missing FFmpeg symbol: ")+name);
        return reinterpret_cast<T>(p);
    }
};
struct Api {
    Library util{L"avutil-60.dll"};
    Library codec{L"avcodec-62.dll"};
#define UTIL(n) decltype(&::n) n=util.get<decltype(&::n)>(#n)
#define CODEC(n) decltype(&::n) n=codec.get<decltype(&::n)>(#n)
    UTIL(avutil_version); UTIL(av_strerror); UTIL(av_opt_set);
    UTIL(av_frame_alloc); UTIL(av_frame_free); UTIL(av_frame_get_buffer); UTIL(av_frame_make_writable);
    UTIL(av_buffer_ref); UTIL(av_buffer_unref); UTIL(av_hwdevice_ctx_create);
    UTIL(av_hwframe_ctx_alloc); UTIL(av_hwframe_ctx_init); UTIL(av_hwframe_get_buffer); UTIL(av_hwframe_transfer_data);
    CODEC(avcodec_version); CODEC(avcodec_find_encoder_by_name); CODEC(avcodec_alloc_context3);
    CODEC(avcodec_free_context); CODEC(avcodec_open2); CODEC(avcodec_send_frame); CODEC(avcodec_receive_packet);
    CODEC(av_packet_alloc); CODEC(av_packet_free); CODEC(av_packet_unref);
#undef UTIL
#undef CODEC
    Api() {
        if((avcodec_version()>>16)!=LIBAVCODEC_VERSION_MAJOR || (avutil_version()>>16)!=LIBAVUTIL_VERSION_MAJOR)
            throw std::runtime_error("Bundled FFmpeg ABI version mismatch");
    }
    void check(int result,const char* stage) {
        if(result>=0) return;
        char error[256]{};av_strerror(result,error,sizeof(error));
        throw std::runtime_error(std::string(stage)+": "+error+" ("+std::to_string(result)+")");
    }
};
struct Session {
    Api api;
    AVCodecContext* context=nullptr;
    AVFrame* cpu=nullptr;
    AVFrame* gpu=nullptr;
    AVPacket* packet=nullptr;
    AVBufferRef* device=nullptr;
    int width=0,height=0;
    int64_t sequence=0;
    ~Session() {
        api.av_packet_free(&packet);api.av_frame_free(&gpu);api.av_frame_free(&cpu);
        api.avcodec_free_context(&context);api.av_buffer_unref(&device);
    }
    void option(const char* key,const char* value) {api.check(api.av_opt_set(context->priv_data,key,value,0),key);}
    void initialize(const char* name,int w,int h,int fps,int bitrate) {
        if(w<16 || h<16 || w>3840 || h>2160 || w%2 || h%2 || fps<5 || fps>60)
            throw std::runtime_error("Invalid FFmpeg dimensions or frame rate");
        const bool nvidia=std::strcmp(name,"h264_nvenc")==0;
        const bool amd=std::strcmp(name,"h264_amf")==0;
        const bool intel=std::strcmp(name,"h264_qsv")==0;
        const bool software=std::strcmp(name,"h264_mf")==0;
        if(!nvidia && !amd && !intel && !software) throw std::runtime_error("Unsupported FFmpeg encoder");
        const AVCodec* codec=api.avcodec_find_encoder_by_name(name);
        if(!codec) throw std::runtime_error(std::string("FFmpeg encoder not built: ")+name);
        context=api.avcodec_alloc_context3(codec);
        cpu=api.av_frame_alloc();packet=api.av_packet_alloc();
        if(!context || !cpu || !packet) throw std::runtime_error("FFmpeg allocation failed");
        width=w;height=h;
        context->width=w;context->height=h;context->pix_fmt=AV_PIX_FMT_NV12;
        context->time_base={1,fps};context->framerate={fps,1};
        // Allow short complexity bursts without building a long rate-control buffer.
        context->bit_rate=bitrate;context->rc_max_rate=bitrate;context->rc_buffer_size=std::max(bitrate/fps,std::min(bitrate/fps*3,bitrate/10));
        context->gop_size=fps*2;context->max_b_frames=0;context->flags|=AV_CODEC_FLAG_LOW_DELAY;
        context->color_range=AVCOL_RANGE_MPEG;context->colorspace=AVCOL_SPC_SMPTE170M;
        context->color_primaries=AVCOL_PRI_SMPTE170M;context->color_trc=AVCOL_TRC_SMPTE170M;
        if(nvidia) {
            // RustDesk's RAM route uses D3D11 hardware frames, avoiding CUDA contexts.
            ComPtr<IDXGIFactory1> factory;
            if(FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory)))) throw std::runtime_error("DXGI factory failed");
            std::string adapter_index;
            for(UINT i=0;;++i) {
                ComPtr<IDXGIAdapter1> adapter;
                HRESULT result=factory->EnumAdapters1(i,&adapter);
                if(result==DXGI_ERROR_NOT_FOUND) break;
                if(FAILED(result)) throw std::runtime_error("DXGI adapter enumeration failed");
                DXGI_ADAPTER_DESC1 description{};
                if(SUCCEEDED(adapter->GetDesc1(&description)) && description.VendorId==0x10de && !(description.Flags&DXGI_ADAPTER_FLAG_SOFTWARE)) {
                    adapter_index=std::to_string(i);break;
                }
            }
            if(adapter_index.empty()) throw std::runtime_error("No NVIDIA DXGI adapter");
            api.check(api.av_hwdevice_ctx_create(&device,AV_HWDEVICE_TYPE_D3D11VA,adapter_index.c_str(),nullptr,0),"Create FFmpeg D3D11 device");
            context->hw_frames_ctx=api.av_hwframe_ctx_alloc(device);
            if(!context->hw_frames_ctx) throw std::runtime_error("Allocate FFmpeg hardware frames failed");
            auto* frames=reinterpret_cast<AVHWFramesContext*>(context->hw_frames_ctx->data);
            frames->format=AV_PIX_FMT_D3D11;frames->sw_format=AV_PIX_FMT_NV12;
            frames->width=w;frames->height=h;
            // D3D11VA maps this to Texture2D.ArraySize, not to a count of
            // independent textures. Match RustDesk's single NV12 input surface;
            // FFmpeg manages NVENC's output surfaces separately.
            frames->initial_pool_size=1;
            const std::string frame_stage="Initialize FFmpeg D3D11 NV12 frames "+std::to_string(w)+"x"+std::to_string(h)+" array_size=1";
            api.check(api.av_hwframe_ctx_init(context->hw_frames_ctx),frame_stage.c_str());
            gpu=api.av_frame_alloc();if(!gpu)throw std::runtime_error("Allocate hardware frame failed");
            api.check(api.av_hwframe_get_buffer(context->hw_frames_ctx,gpu,0),"Get hardware frame");
            context->pix_fmt=AV_PIX_FMT_D3D11;
            option("preset","p3");option("tune","ull");option("rc","cbr");option("delay","0");option("zerolatency","1");
        } else if(amd) {
            option("usage","ultralowlatency");option("quality","speed");option("rc","cbr");
        } else if(intel) {
            option("preset","veryfast");option("async_depth","1");option("look_ahead","0");
        } else {
            option("hw_encoding","0");
        }
        api.check(api.avcodec_open2(context,codec,nullptr),"Open FFmpeg hardware encoder");
        cpu->format=AV_PIX_FMT_NV12;cpu->width=w;cpu->height=h;
        api.check(api.av_frame_get_buffer(cpu,32),"Allocate CPU input frame");
    }
    size_t encode(const uint8_t* data,size_t length,uint8_t* output,size_t capacity) {
        if(!output)throw std::runtime_error("Invalid FFmpeg output buffer");
        if(!data && length==0) {
            api.check(api.avcodec_send_frame(context,nullptr),"Drain FFmpeg encoder");
        } else {
            if(!data || length!=size_t(width)*height*3/2) throw std::runtime_error("Invalid FFmpeg NV12 buffer length");
            api.check(api.av_frame_make_writable(cpu),"Make CPU frame writable");
            for(int y=0;y<height;++y) std::memcpy(cpu->data[0]+size_t(y)*cpu->linesize[0],data+size_t(y)*width,width);
            const uint8_t* uv=data+size_t(width)*height;
            for(int y=0;y<height/2;++y) std::memcpy(cpu->data[1]+size_t(y)*cpu->linesize[1],uv+size_t(y)*width,width);
            AVFrame* input=cpu;
            if(gpu) {
                api.check(api.av_frame_make_writable(gpu),"Make GPU frame writable");
                api.check(api.av_hwframe_transfer_data(gpu,cpu,0),"Upload FFmpeg hardware frame");
                input=gpu;
            }
            input->pts=sequence++;
            api.check(api.avcodec_send_frame(context,input),"Send FFmpeg frame");
        }
        size_t written=0;
        for(;;) {
            const int result=api.avcodec_receive_packet(context,packet);
            if(result==AVERROR(EAGAIN) || result==AVERROR_EOF)break;
            api.check(result,"Receive FFmpeg packet");
            if(packet->size<0 || size_t(packet->size)>capacity-written) {
                api.av_packet_unref(packet);throw std::runtime_error("FFmpeg output exceeds packet limit");
            }
            if(packet->size)std::memcpy(output+written,packet->data,packet->size);
            written+=packet->size;api.av_packet_unref(packet);
        }
        return written;
    }
};
extern "C" void* kokoda_ffmpeg_create(const char* name,uint32_t w,uint32_t h,uint32_t fps,uint32_t bitrate,char* error,size_t cap) noexcept {
    try {auto session=std::make_unique<Session>();session->initialize(name,w,h,fps,bitrate);return session.release();}
    catch(const std::exception& e){error_text(error,cap,e.what());return nullptr;}
    catch(...){error_text(error,cap,"Unknown FFmpeg initialization failure");return nullptr;}
}
extern "C" int kokoda_ffmpeg_encode(void* handle,const uint8_t* input,size_t length,uint8_t* output,size_t capacity,size_t* written,char* error,size_t cap) noexcept {
    try {if(!handle || !written)throw std::runtime_error("Invalid FFmpeg handle");*written=static_cast<Session*>(handle)->encode(input,length,output,capacity);return 0;}
    catch(const std::exception& e){error_text(error,cap,e.what());return -1;}
    catch(...){error_text(error,cap,"Unknown FFmpeg encoding failure");return -1;}
}
extern "C" void kokoda_ffmpeg_destroy(void* handle) noexcept {delete static_cast<Session*>(handle);}
