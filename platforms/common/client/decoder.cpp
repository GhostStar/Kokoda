#include "decoder.h"
#include <QBuffer>
#include <QElapsedTimer>
#include <QImageReader>
#include <QSemaphore>
#include <stdexcept>
extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libswscale/swscale.h>
}
namespace kokoda {
static QSemaphore decodeSlots(2);
// Accessed only by Player objects on the GUI thread.
static qint64 totalQueuedBytes = 0;
struct Decoder::State {
    AVCodecContext *codec = nullptr;
    AVBufferRef *device = nullptr;
    SwsContext *scale = nullptr;
    qint64 epoch = -1;
    qint64 generation = -1;
    AVPixelFormat hardware = AV_PIX_FMT_NONE;
    ~State() {
        avcodec_free_context(&codec);
        av_buffer_unref(&device);
        sws_freeContext(scale);
    }
    static AVPixelFormat choose(AVCodecContext *c, const AVPixelFormat *formats) {
        auto wanted = static_cast<State *>(c->opaque)->hardware;
        for (auto f = formats; *f != AV_PIX_FMT_NONE; ++f)
            if (*f == wanted)
                return *f;
        return formats[0];
    }
    void open(bool accelerate) {
        const auto *implementation = avcodec_find_decoder(AV_CODEC_ID_H264);
        codec = avcodec_alloc_context3(implementation);
        if (!codec)
            throw std::runtime_error("decoder allocation");
        codec->thread_count = 1;
        codec->max_pixels = 33177600;
        codec->opaque = this;
        if (accelerate) {
#ifdef Q_OS_WIN
            const auto type = AV_HWDEVICE_TYPE_D3D11VA;
#else
            const auto type = AV_HWDEVICE_TYPE_VAAPI;
#endif
            for (int n = 0; const auto *config = avcodec_get_hw_config(implementation, n); ++n) {
                if (config->device_type == type &&
                    (config->methods & AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX)) {
                    if (av_hwdevice_ctx_create(&device, type, nullptr, nullptr, 0) >= 0) {
                        hardware = config->pix_fmt;
                        codec->hw_device_ctx = av_buffer_ref(device);
                        codec->get_format = choose;
                    }
                    break;
                }
            }
        }
        if (avcodec_open2(codec, implementation, nullptr) < 0)
            throw std::runtime_error("decoder initialization");
    }
};
QImage checkedImage(const QByteArray &data) {
    if (data.isEmpty() || data.size() > 32 * 1024 * 1024)
        return {};
    QBuffer buffer;
    buffer.setData(data);
    buffer.open(QIODevice::ReadOnly);
    QImageReader reader(&buffer);
    const auto size = reader.size();
    if (size.width() <= 0 || size.height() <= 0 || qint64(size.width()) * size.height() > 33177600)
        return {};
    return reader.read();
}
Decoder::Decoder() : state(new State) {}
Decoder::~Decoder() = default;
void Decoder::decode(QJsonObject packet) {
    decodeSlots.acquire();
    QSemaphoreReleaser slot(&decodeSlots);
    QElapsedTimer elapsed;
    elapsed.start();
    QImage output;
    QString error;
    try {
        auto bytes = QByteArray::fromBase64(packet["image"].toString().toLatin1(),
                                            QByteArray::AbortOnBase64DecodingErrors);
        if (bytes.isEmpty() || bytes.size() > 32 * 1024 * 1024)
            throw std::runtime_error("invalid frame");
        if (packet["format"] != "h264")
            output = checkedImage(bytes);
        else {
            auto epoch = packet["stream_epoch"].toInteger();
            auto generation = packet["_local_generation"].toInteger();
            if (state->epoch != epoch || state->generation != generation) {
                state.reset(new State);
                state->epoch = epoch;
                state->generation = generation;
                state->open(!qEnvironmentVariableIsSet("KOKODA_SOFTWARE_DECODE"));
            }
            AVPacket *encoded = av_packet_alloc();
            AVFrame *frame = av_frame_alloc(), *software = av_frame_alloc(),
                    *rgba = av_frame_alloc();
            if (!encoded || !frame || !software || !rgba) {
                av_packet_free(&encoded);
                av_frame_free(&frame);
                av_frame_free(&software);
                av_frame_free(&rgba);
                throw std::runtime_error("frame allocation");
            }
            auto cleanup = [&] {
                av_packet_free(&encoded);
                av_frame_free(&frame);
                av_frame_free(&software);
                av_frame_free(&rgba);
            };
            if (av_new_packet(encoded, int(bytes.size())) < 0) {
                cleanup();
                throw std::runtime_error("packet allocation");
            }
            memcpy(encoded->data, bytes.constData(), bytes.size());
            auto status = avcodec_send_packet(state->codec, encoded);
            if (status < 0 && state->device) {
                state.reset(new State);
                state->epoch = epoch;
                state->generation = generation;
                state->open(false);
                status = avcodec_send_packet(state->codec, encoded);
            }
            if (status < 0) {
                cleanup();
                throw std::runtime_error("H264 packet");
            }
            while ((status = avcodec_receive_frame(state->codec, frame)) >= 0) {
                AVFrame *pixels = frame;
                if (frame->hw_frames_ctx) {
                    if (av_hwframe_transfer_data(software, frame, 0) < 0) {
                        cleanup();
                        throw std::runtime_error("hardware frame transfer");
                    }
                    pixels = software;
                }
                if (pixels->width <= 0 || pixels->height <= 0 ||
                    qint64(pixels->width) * pixels->height > 33177600) {
                    cleanup();
                    throw std::runtime_error("frame dimensions");
                }
                // libswscale SIMD paths require aligned, padded plane storage. A
                // tightly allocated QImage does not provide FFmpeg's padding.
                av_frame_unref(rgba);
                rgba->format = AV_PIX_FMT_RGBA;
                rgba->width = pixels->width;
                rgba->height = pixels->height;
                if (av_frame_get_buffer(rgba, 64) < 0) {
                    cleanup();
                    throw std::runtime_error("pixel allocation");
                }
                state->scale = sws_getCachedContext(state->scale, pixels->width, pixels->height,
                                                    AVPixelFormat(pixels->format), pixels->width,
                                                    pixels->height, AV_PIX_FMT_RGBA, SWS_BILINEAR,
                                                    nullptr, nullptr, nullptr);
                if (!state->scale) {
                    cleanup();
                    throw std::runtime_error("pixel conversion");
                }
                const auto *coeff = sws_getCoefficients(SWS_CS_ITU601);
                sws_setColorspaceDetails(state->scale, coeff, 0, coeff, 1, 0, 1 << 16, 1 << 16);
                if (sws_scale(state->scale, pixels->data, pixels->linesize, 0, pixels->height,
                              rgba->data, rgba->linesize) != pixels->height) {
                    cleanup();
                    throw std::runtime_error("pixel conversion");
                }
                output = QImage(rgba->data[0], rgba->width, rgba->height, rgba->linesize[0],
                                QImage::Format_RGBA8888)
                             .copy();
                av_frame_unref(frame);
                av_frame_unref(software);
            }
            cleanup();
            if (status != AVERROR(EAGAIN) && status != AVERROR_EOF)
                throw std::runtime_error("H264 decode");
        }
        if (!output.isNull() && !packet["alpha_png"].toString().isEmpty()) {
            auto mask =
                checkedImage(QByteArray::fromBase64(packet["alpha_png"].toString().toLatin1()));
            if (mask.isNull())
                throw std::runtime_error("alpha mask");
            mask = mask.scaled(output.size(), Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
                       .convertToFormat(QImage::Format_Grayscale8);
            output = output.convertToFormat(QImage::Format_RGBA8888);
            for (int y = 0; y < output.height(); ++y) {
                auto *row = output.scanLine(y);
                auto *alpha = mask.constScanLine(y);
                for (int x = 0; x < output.width(); ++x)
                    row[x * 4 + 3] = alpha[x];
            }
        }
        if (output.isNull() && packet["format"] != "h264")
            throw std::runtime_error("image decode");
    } catch (const std::exception &e) {
        error = QString::fromUtf8(e.what());
    }
    packet.remove("image");
    packet.remove("alpha_png");
    emit decoded(packet, output, elapsed.nsecsElapsed() / 1e6, error);
}
Player::Player(QObject *parent) : QObject(parent), decoder(new Decoder) {
    decoder->moveToThread(&thread);
    connect(&thread, &QThread::finished, decoder, &QObject::deleteLater);
    connect(this, &Player::enqueue, decoder, &Decoder::decode);
    connect(decoder, &Decoder::decoded, this,
            [this](QJsonObject p, QImage image, double ms, QString error) {
                --queued;
                const auto bytes = p.take("_encoded_bytes").toInteger();
                bytesQueued -= bytes;
                totalQueuedBytes -= bytes;
                emit decoded(p, image, ms, error);
            });
    thread.start();
}
Player::~Player() {
    totalQueuedBytes -= bytesQueued;
    thread.quit();
    thread.wait();
}
bool Player::submit(const QJsonObject &packet) {
    const qint64 bytes =
        2 * (packet["image"].toString().size() + packet["alpha_png"].toString().size());
    if (queued >= 3 || totalQueuedBytes + bytes > 96 * 1024 * 1024)
        return false;
    ++queued;
    bytesQueued += bytes;
    totalQueuedBytes += bytes;
    auto job = packet;
    job["_encoded_bytes"] = bytes;
    emit enqueue(job);
    return true;
}
} // namespace kokoda
