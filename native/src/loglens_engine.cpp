#include "loglens_engine.h"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <fstream>
#include <limits>
#include <unordered_map>
#include <utility>
#include <vector>

namespace {
uint16_t u16e(const uint8_t* b, bool le) {
    return le ? uint16_t(b[0] | (uint16_t(b[1]) << 8))
              : uint16_t((uint16_t(b[0]) << 8) | b[1]);
}
uint32_t u32e(const uint8_t* b, bool le) {
    if (le) return uint32_t(b[0]) | (uint32_t(b[1]) << 8) | (uint32_t(b[2]) << 16) | (uint32_t(b[3]) << 24);
    return (uint32_t(b[0]) << 24) | (uint32_t(b[1]) << 16) | (uint32_t(b[2]) << 8) | uint32_t(b[3]);
}
bool read_file(const char* path, std::vector<uint8_t>& out) {
    std::ifstream f(path, std::ios::binary | std::ios::ate);
    if (!f) return false;
    const auto n = f.tellg();
    if (n < 0) return false;
    out.resize(static_cast<size_t>(n));
    f.seekg(0);
    if (!out.empty()) f.read(reinterpret_cast<char*>(out.data()), static_cast<std::streamsize>(out.size()));
    return bool(f) || out.empty();
}
struct PacketTmp {
    uint64_t ts;
    uint32_t link;
    uint32_t iface;
    size_t offset;
    size_t length;
};
struct Iface { uint32_t linktype; double units; int64_t offset_seconds=0; };
uint64_t apply_timestamp_offset(uint64_t ns,int64_t offset) {
    const auto magnitude=offset<0 ? uint64_t(-(offset+1))+1 : uint64_t(offset);
    if(magnitude>UINT64_MAX/1000000000ULL) return offset<0?0:UINT64_MAX;
    const auto delta=magnitude*1000000000ULL;
    if(offset<0) return ns<delta?0:ns-delta;
    return ns>UINT64_MAX-delta?UINT64_MAX:ns+delta;
}

int parse_pcap(const std::vector<uint8_t>& d, size_t max_packets, std::vector<PacketTmp>& packets) {
    if (d.size() < 24) return LL_ERR_FORMAT;
    bool le=false, nano=false;
    const uint8_t* m=d.data();
    if (m[0]==0xd4&&m[1]==0xc3&&m[2]==0xb2&&m[3]==0xa1) { le=true; nano=false; }
    else if (m[0]==0xa1&&m[1]==0xb2&&m[2]==0xc3&&m[3]==0xd4) { le=false; nano=false; }
    else if (m[0]==0x4d&&m[1]==0x3c&&m[2]==0xb2&&m[3]==0xa1) { le=true; nano=true; }
    else if (m[0]==0xa1&&m[1]==0xb2&&m[2]==0x3c&&m[3]==0x4d) { le=false; nano=true; }
    else return LL_ERR_FORMAT;
    const uint32_t link=u32e(m+20,le);
    size_t pos=24;
    while (pos+16<=d.size() && packets.size()<max_packets) {
        const uint64_t sec=u32e(d.data()+pos,le);
        const uint64_t frac=u32e(d.data()+pos+4,le);
        const size_t incl=u32e(d.data()+pos+8,le);
        pos+=16;
        if (pos+incl>d.size()) break;
        const uint64_t ns=sec*1000000000ULL+(nano?frac:frac*1000ULL);
        packets.push_back({ns,link,0,pos,incl});
        pos+=incl;
    }
    return LL_OK;
}
int parse_pcapng(const std::vector<uint8_t>& d, size_t max_packets, std::vector<PacketTmp>& packets) {
    size_t pos=0; bool le=true; std::vector<Iface> ifaces;
    while (pos+12<=d.size() && packets.size()<max_packets) {
        const uint8_t* b=d.data()+pos;
        if (b[0]==0x0A&&b[1]==0x0D&&b[2]==0x0D&&b[3]==0x0A) {
            if (pos+28>d.size()) break;
            const uint8_t* bom=d.data()+pos+8;
            if (bom[0]==0x4d&&bom[1]==0x3c&&bom[2]==0x2b&&bom[3]==0x1a) le=true;
            else if (bom[0]==0x1a&&bom[1]==0x2b&&bom[2]==0x3c&&bom[3]==0x4d) le=false;
            else return LL_ERR_FORMAT;
            const size_t len=u32e(d.data()+pos+4,le);
            if (len<28||pos+len>d.size()) break;
            ifaces.clear(); pos+=len; continue;
        }
        const uint32_t type=u32e(b,le);
        const size_t len=u32e(d.data()+pos+4,le);
        if (len<12||pos+len>d.size()) break;
        if (type==1 && len>=20) {
            const uint32_t link=u16e(d.data()+pos+8,le);
            double units=1e6; int64_t offset_seconds=0;
            size_t o=pos+16, end=pos+len-4;
            while (o+4<=end) {
                const uint16_t code=u16e(d.data()+o,le);
                const size_t olen=u16e(d.data()+o+2,le); o+=4;
                if (o+olen>end) break;
                if (code==9&&olen>=1) {
                    const uint8_t v=d[o];
                    units=(v&0x80)?std::pow(2.0,double(v&0x7f)):std::pow(10.0,double(v));
                }
                if(code==14 && olen==8) {
                    const auto* v=d.data()+o;
                    uint64_t bits=le ? uint64_t(u32e(v,true))|(uint64_t(u32e(v+4,true))<<32)
                                     : (uint64_t(u32e(v,false))<<32)|u32e(v+4,false);
                    std::memcpy(&offset_seconds,&bits,sizeof(bits));
                }
                o+=(olen+3)&~size_t(3);
            }
            ifaces.push_back({link,units,offset_seconds});
        } else if (type==6 && len>=32) {
            const size_t iid=u32e(d.data()+pos+8,le);
            const uint64_t hi=u32e(d.data()+pos+12,le), lo=u32e(d.data()+pos+16,le);
            const size_t cap=u32e(d.data()+pos+20,le), start=pos+28;
            if (start+cap<=pos+len-4) {
                const Iface iface=iid<ifaces.size()?ifaces[iid]:Iface{1,1e6};
                const uint64_t ticks=(hi<<32)|lo;
                uint64_t ns;
                // Integer arithmetic where the tick rate relates to 1 GHz by an integer
                // factor: a double cannot hold a ~1.7e18 ns epoch exactly.
                const double u=iface.units;
                if (u>0 && u<=1e9 && std::floor(1e9/u)==1e9/u) ns=ticks*uint64_t(1e9/u);
                else if (u>1e9 && std::floor(u/1e9)==u/1e9) ns=ticks/uint64_t(u/1e9);
                else ns=uint64_t((double(ticks)/u)*1e9);
                packets.push_back({apply_timestamp_offset(ns,iface.offset_seconds),iface.linktype,uint32_t(iid),start,cap});
            }
        } else if (type==3 && len>=16) {
            const size_t start=pos+12,end=pos+len-4;
            const Iface iface=ifaces.empty()?Iface{1,1e6}:ifaces.front();
            packets.push_back({0,iface.linktype,0,start,end-start});
        }
        pos+=len;
    }
    return LL_OK;
}
struct Key {
    uint32_t ch,id; uint8_t bus;
    bool operator==(const Key& o) const { return ch==o.ch&&id==o.id&&bus==o.bus; }
};
struct KeyHash {
    size_t operator()(const Key& k) const {
        return (size_t(k.ch)*1315423911u)^(size_t(k.id)*2654435761u)^k.bus;
    }
};
struct PayloadState { uint8_t n=0; uint8_t p[64]{}; };
}

extern "C" {
const char* ll_engine_version() { return "LogLens C++ Native Engine 0.26.0"; }

int32_t ll_read_capture(const char* path, size_t max_packets, LLCapture* out) {
    if (!path||!out) return LL_ERR_ARGUMENT;
    *out={};
    try {
        std::vector<uint8_t> file;
        if (!read_file(path,file)) return LL_ERR_IO;
        if (file.size()<4) return LL_ERR_FORMAT;
        std::vector<PacketTmp> tmp;
        tmp.reserve(std::min<size_t>(max_packets,1000000));
        int rc=(file[0]==0x0A&&file[1]==0x0D&&file[2]==0x0D&&file[3]==0x0A)
            ? parse_pcapng(file,max_packets,tmp) : parse_pcap(file,max_packets,tmp);
        if (rc!=LL_OK) return rc;
        size_t bytes=0; for (const auto& p:tmp) bytes+=p.length;
        out->packets=new LLPacket[tmp.size()];
        out->data=new uint8_t[bytes?bytes:1];
        out->packet_count=tmp.size(); out->data_size=bytes;
        size_t off=0;
        for (size_t i=0;i<tmp.size();++i) {
            const auto& p=tmp[i];
            out->packets[i]={p.ts,p.link,p.iface,uint32_t(off),uint32_t(p.length)};
            if (p.length) std::memcpy(out->data+off,file.data()+p.offset,p.length);
            off+=p.length;
        }
        return LL_OK;
    } catch (...) {
        ll_free_capture(out);
        return LL_ERR_MEMORY;
    }
}
void ll_free_capture(LLCapture* c) {
    if (!c) return;
    delete[] c->packets; delete[] c->data; *c={};
}

int32_t ll_analyze_frames(const LLFrameKey* frames,size_t count,LLNativeStats* out) {
    if ((!frames&&count)||!out) return LL_ERR_ARGUMENT;
    std::unordered_map<Key,PayloadState,KeyHash> state;
    state.reserve(std::min<size_t>(count,200000));
    uint64_t changed=0, stat=0;
    for(size_t i=0;i<count;++i) {
        const auto& f=frames[i]; Key k{f.channel_id,f.identifier,f.bus_type};
        auto it=state.find(k);
        if(it==state.end()) {
            PayloadState ps; ps.n=std::min<uint8_t>(f.payload_length,64);
            std::memcpy(ps.p,f.payload,ps.n); state.emplace(k,ps); changed++;
        } else {
            const uint8_t n=std::min<uint8_t>(f.payload_length,64);
            const bool diff=it->second.n!=n||std::memcmp(it->second.p,f.payload,n)!=0;
            if(diff){changed++;it->second.n=n;std::memcpy(it->second.p,f.payload,n);} else stat++;
        }
    }
    *out={uint64_t(count),changed,stat,uint64_t(state.size())};
    return LL_OK;
}

size_t ll_downsample_minmax(const double* t,const double* v,size_t n,size_t target,double* ot,double* ov,size_t cap) {
    if(!t||!v||!ot||!ov||!n||!target||!cap) return 0;
    if(n<=target) { const size_t m=std::min(n,cap); for(size_t i=0;i<m;++i){ot[i]=t[i];ov[i]=v[i];} return m; }
    const size_t buckets=std::max<size_t>(1,target/2);
    const double step=double(n)/double(buckets); size_t outn=0;
    for(size_t b=0;b<buckets&&outn+2<=cap;++b){
        size_t a=size_t(std::floor(b*step)), z=std::min(n,size_t(std::floor((b+1)*step)));
        if(z<=a) z=std::min(n,a+1);
        size_t imin=a,imax=a;
        for(size_t i=a+1;i<z;++i){if(v[i]<v[imin])imin=i;if(v[i]>v[imax])imax=i;}
        if(imin<=imax){ot[outn]=t[imin];ov[outn++]=v[imin]; if(imax!=imin&&outn<cap){ot[outn]=t[imax];ov[outn++]=v[imax];}}
        else {ot[outn]=t[imax];ov[outn++]=v[imax]; if(outn<cap){ot[outn]=t[imin];ov[outn++]=v[imin];}}
    }
    return outn;
}
}


extern "C" int32_t ll_build_frame_store(const LLFrameKey* frames, size_t count, LLFrameStore* out) {
 if (!out || (count && !frames)) return LL_ERR_ARGUMENT;
 std::memset(out,0,sizeof(*out)); if(!count) return LL_OK;
 auto* recs=static_cast<LLCompactFrame*>(std::calloc(count,sizeof(LLCompactFrame))); if(!recs) return LL_ERR_MEMORY;
 size_t bytes=0; for(size_t i=0;i<count;i++) bytes+=frames[i].payload_length;
 auto* payload=bytes?static_cast<uint8_t*>(std::malloc(bytes)):nullptr;
 if(bytes && !payload){std::free(recs);return LL_ERR_MEMORY;}
 size_t off=0;
 for(size_t i=0;i<count;i++){ auto &o=recs[i]; const auto &f=frames[i]; o.timestamp_ns=f.timestamp_ns; o.payload_offset=(uint32_t)off; o.identifier=f.identifier; o.channel_id=f.channel_id; o.payload_length=f.payload_length; o.bus_type=f.bus_type; if(f.payload_length){std::memcpy(payload+off,f.payload,f.payload_length);off+=f.payload_length;} }
 out->frames=recs; out->payload=payload; out->frame_count=count; out->payload_size=bytes; return LL_OK;
}
extern "C" void ll_free_frame_store(LLFrameStore* s){ if(!s)return; std::free(s->frames); std::free(s->payload); std::memset(s,0,sizeof(*s)); }
extern "C" size_t ll_query_frame_range(const LLFrameStore* s,size_t start,size_t count,LLCompactFrame* out,size_t cap){ if(!s||!out||start>=s->frame_count||!cap)return 0; size_t n=std::min(count,std::min(cap,s->frame_count-start)); std::memcpy(out,s->frames+start,n*sizeof(LLCompactFrame)); return n; }
