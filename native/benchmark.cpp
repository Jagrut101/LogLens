#include "include/loglens_engine.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <vector>
int main(int argc,char** argv) {
    if(argc<2){std::fprintf(stderr,"usage: loglens_native_benchmark capture.pcapng [max_packets]\n");return 2;}
    const size_t maxp=argc>2?std::strtoull(argv[2],nullptr,10):100000000ULL;
    LLCapture c{};
    auto a=std::chrono::steady_clock::now();
    int rc=ll_read_capture(argv[1],maxp,&c);
    auto b=std::chrono::steady_clock::now();
    if(rc){std::fprintf(stderr,"native reader error %d\n",rc);return 1;}
    double sec=std::chrono::duration<double>(b-a).count();
    std::printf("%s\npackets=%zu payload_bytes=%zu time=%.3fs rate=%.2f Mpacket/s\n",
        ll_engine_version(),c.packet_count,c.data_size,sec,sec>0?double(c.packet_count)/sec/1e6:0.0);
    ll_free_capture(&c);
}
