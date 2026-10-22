//! Hardware-independent CoreMIDI ingress check. The temporary destination belongs
//! only to this test; messages never go to the running app or external devices.
use super::*;
use crate::{
    midi::mapping::{Controller, Mapping},
    modules::{ModuleKind, ModulePane},
    parameter::schema::{ParameterContext, ParameterSpec},
};

unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    fn CFRelease(object: *const c_void);
    fn MIDIClientCreate(
        name: *const c_void,
        notify: *const c_void,
        context: *mut c_void,
        client: *mut u32,
    ) -> i32;
    fn MIDIClientDispose(client: u32) -> i32;
    fn MIDIOutputPortCreate(client: u32, name: *const c_void, port: *mut u32) -> i32;
    fn MIDIGetNumberOfDestinations() -> usize;
    fn MIDIGetDestination(index: usize) -> u32;
    fn MIDIPacketListInit(list: *mut c_void) -> *mut c_void;
    fn MIDIPacketListAdd(
        list: *mut c_void,
        size: usize,
        packet: *mut c_void,
        timestamp: u64,
        length: usize,
        bytes: *const u8,
    ) -> *mut c_void;
    fn MIDISend(port: u32, destination: u32, list: *const c_void) -> i32;
}

struct Client(u32);
impl Drop for Client {
    fn drop(&mut self) {
        unsafe { MIDIClientDispose(self.0) };
    }
}
fn destinations() -> BTreeSet<u32> {
    unsafe {
        (0..MIDIGetNumberOfDestinations())
            .map(|index| MIDIGetDestination(index))
            .collect()
    }
}

#[test]
#[ignore = "requires the macOS CoreMIDI service"]
fn virtual_coremidi_packets_drive_absolute_parameter_mapping() {
    let before = destinations();
    let preferences = Arc::new(ArcSwap::from_pointee(Preferences::default()));
    let mut backend = Backend::new(preferences.clone()).unwrap();
    let created: Vec<_> = destinations().difference(&before).copied().collect();
    assert_eq!(created.len(), 1, "expected exactly our new virtual input");
    let destination = created[0];
    let mut client = Client(0);
    let mut port = 0;
    unsafe {
        let name = CFStringCreateWithCString(
            std::ptr::null(),
            c"Symphos MIDI test sender".as_ptr(),
            0x0800_0100, // kCFStringEncodingUTF8
        );
        assert!(!name.is_null());
        let created = MIDIClientCreate(name, std::ptr::null(), std::ptr::null_mut(), &mut client.0);
        let opened = if created == 0 {
            MIDIOutputPortCreate(client.0, name, &mut port)
        } else {
            created
        };
        CFRelease(name);
        assert_eq!(created, 0);
        assert_eq!(opened, 0);
    }
    let mut engine = Engine::default();
    engine.ports(backend.ports(), &preferences.load());
    let ctx = egui::Context::default();
    let mut panes = vec![ModulePane::new(ModuleKind::Waterfall)];
    let mut mappings = vec![Mapping {
        target: ParameterContext {
            pane: panes[0].instance_id().into(),
            kind: ModuleKind::Waterfall,
        },
        parameter: ParameterSpec {
            field: "height".into(),
            label: "Height".into(),
            min: 0.0,
            max: 2.0,
            safe_min: 0.0,
            safe_max: 2.0,
            default: 0.85,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        },
        input: Binding {
            port: Some(VIRTUAL_PORT),
            channel: Some(1),
            message: Message::ControlChange,
            number: Some(17),
        },
    }];
    let mut controller = Controller::default();
    controller.reset(&engine.snapshot);
    for value in [0, 64, 127] {
        let serial = engine.snapshot.serial;
        // u64 storage provides the alignment required by MIDIPacketList. The C
        // initializer/add API writes its platform-specific packet layout.
        let mut storage = [0_u64; 128];
        unsafe {
            let list = storage.as_mut_ptr().cast();
            let packet = MIDIPacketListInit(list);
            let bytes = [0xb0, 17, value];
            assert!(
                !MIDIPacketListAdd(list, 1024, packet, 0, bytes.len(), bytes.as_ptr()).is_null()
            );
            assert_eq!(MIDISend(port, destination, list), 0);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while engine.snapshot.serial == serial {
            backend.drain(&mut engine);
            assert!(
                Instant::now() < deadline,
                "CoreMIDI callback did not deliver CC"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes);
        let actual = crate::macros::get(&panes[0].copy_settings(), "height").unwrap();
        assert!((actual - f64::from(value) / 127.0 * 2.0).abs() < 1e-6);
    }
    assert_eq!(engine.snapshot.dropped, 0);
}
