// openh264 的 C 包装：Rust 只看见四个函数，C++ 的虚表、SFrameBSInfo 那些大
// 结构、NAL 起始码全在这里消化掉。
//
// 输出约定（AVCC / ISO 14496-15）：
//   - 每个 slice NAL 前面加 4 字节大端长度，SPS(7)/PPS(8) 不写进 sample，
//     单独用 sky_h264_parameter_sets 取走放进 avcC。
//   - 一帧可能有多个 slice（openh264 多线程会切），全部按顺序拼进去。
//
// 这里顺手补上 C++ 运行时缺的那几个符号（operator new/delete、__cxa_guard_*、
// __cxa_pure_virtual），所以整个静态库不依赖 libc++，链接时只要 -lc 就够。

#include "codec_api.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

// ---------------------------------------------------------------- C++ 运行时

void* operator new(size_t n) {
    void* p = malloc(n ? n : 1);
    if (!p) abort();
    return p;
}

void* operator new[](size_t n) {
    void* p = malloc(n ? n : 1);
    if (!p) abort();
    return p;
}

void operator delete(void* p) noexcept {
    free(p);
}

void operator delete[](void* p) noexcept {
    free(p);
}

void operator delete(void* p, size_t) noexcept {
    free(p);
}

void operator delete[](void* p, size_t) noexcept {
    free(p);
}

extern "C" int __cxa_guard_acquire(uint64_t* guard) {
    return *(volatile uint8_t*)guard == 0;
}

extern "C" void __cxa_guard_release(uint64_t* guard) {
    *(volatile uint8_t*)guard = 1;
}

extern "C" void __cxa_guard_abort(uint64_t*) {}

extern "C" void __cxa_pure_virtual() {
    abort();
}

// ---------------------------------------------------------------- 编码器

struct SkyH264 {
    ISVCEncoder* enc;
    SSourcePicture pic;
    SFrameBSInfo* info;
    int width;
    int height;
    /// 第一帧编完之后才有：SPS / PPS 的裸 NAL（去掉起始码）。
    uint8_t sps[256];
    int sps_len;
    uint8_t pps[256];
    int pps_len;
};

/// 建一个编码器。`bitrate` 是目标码率（bps），`qp` 只在码率够用时决定画质底。
extern "C" SkyH264* sky_h264_new(int width, int height, int fps, int bitrate) {
    if (width <= 0 || height <= 0 || fps <= 0) {
        return NULL;
    }
    ISVCEncoder* enc = NULL;
    if (WelsCreateSVCEncoder(&enc) != 0 || enc == NULL) {
        return NULL;
    }
    SEncParamExt p;
    memset(&p, 0, sizeof(p));
    if (enc->GetDefaultParams(&p) != 0) {
        WelsDestroySVCEncoder(enc);
        return NULL;
    }

    p.iUsageType = CAMERA_VIDEO_REAL_TIME;
    p.iPicWidth = width;
    p.iPicHeight = height;
    p.iTargetBitrate = bitrate;
    p.iRCMode = RC_BITRATE_MODE;
    p.fMaxFrameRate = (float)fps;
    p.iTemporalLayerNum = 1;
    p.iSpatialLayerNum = 1;
    // 一秒一个 IDR：相册取缩略图、拖动都靠它，码率涨得不多。
    p.uiIntraPeriod = (unsigned int)fps;
    p.iNumRefFrame = 1;
    p.bEnableFrameSkip = true;
    p.iMultipleThreadIdc = 1;
    p.bEnableDenoise = false;
    p.bEnableBackgroundDetection = false;
    p.bEnableAdaptiveQuant = false;
    p.bEnableLongTermReference = false;
    p.bEnableSceneChangeDetect = false;
    // baseline 必须 CAVLC；LOW_COMPLEXITY 让编码器少跑几遍模式选择。
    p.iEntropyCodingModeFlag = 0;
    p.iComplexityMode = LOW_COMPLEXITY;

    p.sSpatialLayers[0].iVideoWidth = width;
    p.sSpatialLayers[0].iVideoHeight = height;
    p.sSpatialLayers[0].fFrameRate = (float)fps;
    p.sSpatialLayers[0].iSpatialBitrate = bitrate;
    p.sSpatialLayers[0].iMaxSpatialBitrate = bitrate;
    p.sSpatialLayers[0].uiProfileIdc = PRO_BASELINE;
    p.sSpatialLayers[0].uiLevelIdc = LEVEL_UNKNOWN;
    p.sSpatialLayers[0].sSliceArgument.uiSliceMode = SM_SINGLE_SLICE;
    p.sSpatialLayers[0].sSliceArgument.uiSliceNum = 1;

    if (enc->InitializeExt(&p) != 0) {
        WelsDestroySVCEncoder(enc);
        return NULL;
    }
    // 0 = 不按帧率丢帧（丢帧我们自己数）。
    int video_format = videoFormatI420;
    enc->SetOption(ENCODER_OPTION_DATAFORMAT, &video_format);

    SkyH264* h = (SkyH264*)calloc(1, sizeof(SkyH264));
    if (!h) {
        WelsDestroySVCEncoder(enc);
        return NULL;
    }
    h->enc = enc;
    h->width = width;
    h->height = height;
    h->info = (SFrameBSInfo*)calloc(1, sizeof(SFrameBSInfo));
    if (!h->info) {
        WelsDestroySVCEncoder(enc);
        free(h);
        return NULL;
    }
    h->pic.iPicWidth = width;
    h->pic.iPicHeight = height;
    h->pic.iColorFormat = videoFormatI420;
    return h;
}

extern "C" void sky_h264_free(SkyH264* h) {
    if (!h) {
        return;
    }
    if (h->enc) {
        h->enc->Uninitialize();
        WelsDestroySVCEncoder(h->enc);
    }
    free(h->info);
    free(h);
}

/// 编一帧。`out` 是 AVCC 字节流（4 字节长度 + NAL），返回值是写进去多少字节，
/// 0 表示这一帧被编码器丢掉了（按帧率控流），负数表示出错。`*key` 是 1 就是 IDR。
extern "C" int sky_h264_encode(SkyH264* h, const uint8_t* y, const uint8_t* u, const uint8_t* v,
                               int stride_y, int stride_uv, int64_t timestamp_ms,
                               uint8_t* out, int cap, int* key) {
    if (!h || !out || cap <= 0) {
        return -1;
    }
    *key = 0;
    h->pic.pData[0] = (unsigned char*)y;
    h->pic.pData[1] = (unsigned char*)u;
    h->pic.pData[2] = (unsigned char*)v;
    h->pic.iStride[0] = stride_y;
    h->pic.iStride[1] = stride_uv;
    h->pic.iStride[2] = stride_uv;
    h->pic.uiTimeStamp = timestamp_ms;

    memset(h->info, 0, sizeof(SFrameBSInfo));
    if (h->enc->EncodeFrame(&h->pic, h->info) != 0) {
        return -1;
    }
    if (h->info->eFrameType == videoFrameTypeSkip || h->info->eFrameType == videoFrameTypeInvalid) {
        return 0;
    }
    if (h->info->eFrameType == videoFrameTypeIDR) {
        *key = 1;
    }

    int wrote = 0;
    for (int i = 0; i < h->info->iLayerNum; i++) {
        const SLayerBSInfo& layer = h->info->sLayerInfo[i];
        int at = 0;
        for (int j = 0; j < layer.iNalCount; j++) {
            int len = layer.pNalLengthInByte[j];
            if (len <= 0) {
                return -1;
            }
            const uint8_t* nal = layer.pBsBuf + at;
            at += len;
            // 起始码可能是 3 或 4 字节，openh264 给的是 4 字节。
            int sc = 0;
            if (len >= 4 && nal[0] == 0 && nal[1] == 0 && nal[2] == 0 && nal[3] == 1) {
                sc = 4;
            } else if (len >= 3 && nal[0] == 0 && nal[1] == 0 && nal[2] == 1) {
                sc = 3;
            }
            if (sc == 0 || len <= sc) {
                continue;
            }
            const uint8_t* body = nal + sc;
            int body_len = len - sc;
            int type = body[0] & 0x1F;
            if (type == 7) {
                if (body_len <= (int)sizeof(h->sps)) {
                    memcpy(h->sps, body, body_len);
                    h->sps_len = body_len;
                }
                continue;
            }
            if (type == 8) {
                if (body_len <= (int)sizeof(h->pps)) {
                    memcpy(h->pps, body, body_len);
                    h->pps_len = body_len;
                }
                continue;
            }
            if (wrote + 4 + body_len > cap) {
                return -1;
            }
            out[wrote++] = (uint8_t)(body_len >> 24);
            out[wrote++] = (uint8_t)(body_len >> 16);
            out[wrote++] = (uint8_t)(body_len >> 8);
            out[wrote++] = (uint8_t)body_len;
            memcpy(out + wrote, body, body_len);
            wrote += body_len;
        }
    }
    return wrote;
}

/// 让下一帧编成 IDR。预热帧（黑帧，编完就扔）会占掉初始化那次 IDR，
/// 之后第一帧真实画面就变成 P 帧 —— P 帧里没有 SPS/PPS，`avcC` 拿不到参数，
/// 相册缩略图会黑。所以在第一帧真实画面之前叫一下这个。
extern "C" int sky_h264_force_idr(SkyH264* h) {
    if (!h || !h->enc) {
        return -1;
    }
    return h->enc->ForceIntraFrame(true, 0) == 0 ? 1 : 0;
}

/// 第一帧编完之后取 SPS / PPS。返回 1 表示这一次拿到了，0 表示还没有。
extern "C" int sky_h264_parameter_sets(SkyH264* h, uint8_t* sps, int sps_cap, int* sps_len,
                                       uint8_t* pps, int pps_cap, int* pps_len) {
    if (!h || h->sps_len <= 0 || h->pps_len <= 0) {
        return 0;
    }
    if (h->sps_len > sps_cap || h->pps_len > pps_cap) {
        return 0;
    }
    memcpy(sps, h->sps, h->sps_len);
    memcpy(pps, h->pps, h->pps_len);
    *sps_len = h->sps_len;
    *pps_len = h->pps_len;
    return 1;
}
