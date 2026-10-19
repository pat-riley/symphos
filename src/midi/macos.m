// CoreMIDI owns its callback thread. Forward bytes to a bounded Rust queue;
// enumeration and connection lifetime stay on the MIDI worker.
#import <CoreMIDI/CoreMIDI.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdlib.h>

typedef void (*Receive)(void *, int32_t, const uint8_t *, size_t);
typedef struct {
    MIDIClientRef client;
    MIDIPortRef input;
    MIDIEndpointRef destination;
    MIDIEndpointRef sources[256];
    int32_t ids[256];
    size_t count;
    Receive receive;
    void *context;
} SymphosMIDI;

static void read_packets(const MIDIPacketList *list, void *context, void *source) {
    SymphosMIDI *m = context;
    const MIDIPacket *packet = &list->packet[0];
    for (UInt32 i = 0; i < list->numPackets; ++i) {
        m->receive(m->context, (int32_t)(intptr_t)source, packet->data, packet->length);
        packet = MIDIPacketNext(packet);
    }
}
static void read_virtual(const MIDIPacketList *list, void *context, void *unused) {
    (void)unused;
    read_packets(list, context, (void *)(intptr_t)INT32_MIN);
}

// MIDI 1.0 byte input intentionally handles both legacy hardware and DAW ports.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
void *symphos_midi_create(Receive receive, void *context, int32_t *error) {
    SymphosMIDI *m = calloc(1, sizeof(*m));
    if (!m) { *error = -1; return NULL; }
    m->receive = receive;
    m->context = context;
    *error = MIDIClientCreate(CFSTR("Symphos"), NULL, NULL, &m->client);
    if (!*error) *error = MIDIInputPortCreate(m->client, CFSTR("Symphos input"), read_packets, m, &m->input);
    if (!*error) *error = MIDIDestinationCreate(m->client, CFSTR("Symphos Input"), read_virtual, m, &m->destination);
    if (*error) {
        if (m->client) MIDIClientDispose(m->client);
        free(m);
        return NULL;
    }
    return m;
}
#pragma clang diagnostic pop

size_t symphos_midi_refresh(void *handle) {
    SymphosMIDI *m = handle;
    MIDIEndpointRef endpoints[256];
    int32_t ids[256];
    size_t count = 0;
    for (ItemCount i = 0; i < MIDIGetNumberOfSources() && count < 256; ++i) {
        MIDIEndpointRef source = MIDIGetSource(i);
        SInt32 uid = 0, offline = 0;
        if (MIDIObjectGetIntegerProperty(source, kMIDIPropertyUniqueID, &uid) != noErr) continue;
        MIDIObjectGetIntegerProperty(source, kMIDIPropertyOffline, &offline);
        if (offline) continue;
        bool existing = false;
        for (size_t j = 0; j < m->count; ++j) {
            if (m->sources[j] == source && m->ids[j] == uid) existing = true;
        }
        if (!existing && MIDIPortConnectSource(m->input, source, (void *)(intptr_t)uid) != noErr) continue;
        endpoints[count] = source;
        ids[count++] = uid;
    }
    for (size_t j = 0; j < m->count; ++j) {
        bool found = false;
        for (size_t i = 0; i < count; ++i) {
            if (endpoints[i] == m->sources[j] && ids[i] == m->ids[j]) found = true;
        }
        if (!found) MIDIPortDisconnectSource(m->input, m->sources[j]);
    }
    memcpy(m->sources, endpoints, count * sizeof(*endpoints));
    memcpy(m->ids, ids, count * sizeof(*ids));
    m->count = count;
    return count;
}
int32_t symphos_midi_port(void *handle, size_t index, char *name, size_t capacity) {
    SymphosMIDI *m = handle;
    if (index >= m->count) return 0;
    CFStringRef text = NULL;
    MIDIObjectGetStringProperty(m->sources[index], kMIDIPropertyDisplayName, &text);
    if (!text) MIDIObjectGetStringProperty(m->sources[index], kMIDIPropertyName, &text);
    if (capacity) name[0] = 0;
    if (text) { CFStringGetCString(text, name, capacity, kCFStringEncodingUTF8); CFRelease(text); }
    return m->ids[index];
}
void symphos_midi_destroy(void *handle) {
    SymphosMIDI *m = handle;
    // Disposing the client disposes its ports and stops their callbacks before
    // Rust releases the callback context.
    MIDIClientDispose(m->client);
    free(m);
}
