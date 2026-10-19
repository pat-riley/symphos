use super::*;
use std::{
    ffi::{CStr, c_char, c_void},
    sync::{
        atomic::AtomicUsize,
        mpsc::{Receiver, SyncSender, sync_channel},
    },
};
struct Packet {
    port: i32,
    bytes: [u8; 256],
    len: usize,
}
struct Callback {
    preferences: Arc<ArcSwap<Preferences>>,
    sender: SyncSender<Packet>,
    dropped: AtomicUsize,
}
unsafe extern "C" {
    fn symphos_midi_create(
        receive: extern "C" fn(*mut c_void, i32, *const u8, usize),
        context: *mut c_void,
        error: *mut i32,
    ) -> *mut c_void;
    fn symphos_midi_refresh(handle: *mut c_void) -> usize;
    fn symphos_midi_port(
        handle: *mut c_void,
        index: usize,
        name: *mut c_char,
        capacity: usize,
    ) -> i32;
    fn symphos_midi_destroy(handle: *mut c_void);
}
extern "C" fn receive(context: *mut c_void, port: i32, bytes: *const u8, len: usize) {
    // Context remains pinned in Backend until CoreMIDI has disposed the client.
    let callback = unsafe { &*context.cast::<Callback>() };
    if !callback.preferences.load().enabled.contains(&port) {
        return;
    }
    for chunk in unsafe { std::slice::from_raw_parts(bytes, len) }.chunks(256) {
        let mut packet = Packet {
            port,
            bytes: [0; 256],
            len: chunk.len(),
        };
        packet.bytes[..chunk.len()].copy_from_slice(chunk);
        if callback.sender.try_send(packet).is_err() {
            callback.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(super) struct Backend {
    handle: *mut c_void,
    callback: Box<Callback>,
    receiver: Receiver<Packet>,
}
impl Backend {
    pub fn new(preferences: Arc<ArcSwap<Preferences>>) -> Result<Self, String> {
        let (sender, receiver) = sync_channel(2048);
        let mut callback = Box::new(Callback {
            preferences,
            sender,
            dropped: AtomicUsize::new(0),
        });
        let mut error = 0;
        let handle = unsafe {
            symphos_midi_create(
                receive,
                (&mut *callback as *mut Callback).cast(),
                &mut error,
            )
        };
        if handle.is_null() {
            return Err(format!("CoreMIDI could not start (error {error})."));
        }
        Ok(Self {
            handle,
            callback,
            receiver,
        })
    }
    pub fn ports(&mut self) -> Vec<Port> {
        let mut ports = vec![Port {
            id: VIRTUAL_PORT,
            name: "Symphos Input · virtual".into(),
        }];
        let count = unsafe { symphos_midi_refresh(self.handle) };
        for index in 0..count {
            let mut name = [0 as c_char; 512];
            let id =
                unsafe { symphos_midi_port(self.handle, index, name.as_mut_ptr(), name.len()) };
            ports.push(Port {
                id,
                name: unsafe { CStr::from_ptr(name.as_ptr()) }
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        ports
    }
    pub fn drain(&mut self, engine: &mut Engine) {
        let dropped = self.callback.dropped.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            engine.snapshot.dropped += dropped;
            engine.panic(); // A dropped note-off must never leave a stuck gate.
            while self.receiver.try_recv().is_ok() {}
            return;
        }
        for _ in 0..2048 {
            let Ok(packet) = self.receiver.try_recv() else {
                break;
            };
            engine.bytes(packet.port, &packet.bytes[..packet.len]);
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        unsafe {
            symphos_midi_destroy(self.handle);
        }
    }
}
