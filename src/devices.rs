//! Disk Arbitration notifications with coalescing and periodic reconciliation.
use std::{
    ffi::c_void,
    sync::mpsc::{Receiver, sync_channel},
};
type Ref = *const c_void;
#[link(name = "DiskArbitration", kind = "framework")]
unsafe extern "C" {
    fn DASessionCreate(allocator: Ref) -> Ref;
    fn DARegisterDiskAppearedCallback(
        session: Ref,
        matches: Ref,
        callback: extern "C" fn(Ref, *mut c_void),
        context: *mut c_void,
    );
    fn DARegisterDiskDisappearedCallback(
        session: Ref,
        matches: Ref,
        callback: extern "C" fn(Ref, *mut c_void),
        context: *mut c_void,
    );
    fn DASessionScheduleWithRunLoop(session: Ref, runloop: Ref, mode: Ref);
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: Ref;
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopRun();
}
extern "C" fn changed(_: Ref, context: *mut c_void) {
    // Context remains alive for this dedicated process-lifetime runloop.
    let sender = unsafe { &*(context as *const std::sync::mpsc::SyncSender<()>) };
    let _ = sender.try_send(());
}
pub fn notifications() -> Receiver<()> {
    let (sender, receiver) = sync_channel(1);
    std::thread::spawn(move || unsafe {
        let session = DASessionCreate(std::ptr::null());
        if session.is_null() {
            return;
        }
        let context = Box::into_raw(Box::new(sender)).cast();
        DARegisterDiskAppearedCallback(session, std::ptr::null(), changed, context);
        DARegisterDiskDisappearedCallback(session, std::ptr::null(), changed, context);
        DASessionScheduleWithRunLoop(session, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);
        CFRunLoopRun();
    });
    receiver
}
