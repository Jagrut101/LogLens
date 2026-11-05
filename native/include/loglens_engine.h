#pragma once
#include <cstddef>
#include <cstdint>

#if defined(_WIN32)
#define LL_API __declspec(dllexport)
#else
#define LL_API
#endif

extern "C" {

enum LLStatus : int32_t {
    LL_OK = 0,
    LL_ERR_ARGUMENT = -1,
    LL_ERR_IO = -2,
    LL_ERR_FORMAT = -3,
    LL_ERR_MEMORY = -4
};

struct LLPacket {
    uint64_t timestamp_ns;
    uint32_t linktype;
    uint32_t interface_id;
    uint32_t data_offset;
    uint32_t data_length;
};

struct LLCapture {
    LLPacket* packets;
    uint8_t* data;
    size_t packet_count;
    size_t data_size;
};

struct LLFrameKey {
    uint64_t timestamp_ns;
    uint32_t channel_id;
    uint32_t identifier;
    uint8_t bus_type;
    uint8_t payload_length;
    uint8_t payload[64];
};

struct LLNativeStats {
    uint64_t total_frames;
    uint64_t changed_frames;
    uint64_t static_frames;
    uint64_t unique_keys;
};

LL_API const char* ll_engine_version();
LL_API int32_t ll_read_capture(const char* path, size_t max_packets, LLCapture* out);
LL_API void ll_free_capture(LLCapture* capture);
LL_API int32_t ll_analyze_frames(const LLFrameKey* frames, size_t count, LLNativeStats* out);
LL_API size_t ll_downsample_minmax(
    const double* times,
    const double* values,
    size_t count,
    size_t target_points,
    double* out_times,
    double* out_values,
    size_t out_capacity);

// v0.16 bounded native frame query foundation.
struct LLCompactFrame { uint64_t timestamp_ns; uint32_t payload_offset; uint32_t identifier; uint32_t channel_id; uint16_t payload_length; uint8_t bus_type; uint8_t flags; };
struct LLFrameStore { LLCompactFrame* frames; uint8_t* payload; size_t frame_count; size_t payload_size; };
int32_t ll_build_frame_store(const LLFrameKey* frames, size_t count, LLFrameStore* out);
void ll_free_frame_store(LLFrameStore* store);
size_t ll_query_frame_range(const LLFrameStore* store, size_t start, size_t count, LLCompactFrame* out, size_t out_capacity);
}
