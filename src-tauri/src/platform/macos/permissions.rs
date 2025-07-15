use crate::permissions::PermissionState;
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSString};
use core_foundation::base::TCFType;
use objc::runtime::{BOOL, YES};
use objc::{class, msg_send, sel, sel_impl};

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum AVAuthorizationStatus {
    NotDetermined = 0,
    Restricted = 1,
    Denied = 2,
    Authorized = 3,
}

pub fn check_microphone_permission() -> PermissionState {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let av_capture_device = class!(AVCaptureDevice);
        let media_type_audio = NSString::alloc(nil).init_str("soun");

        let status: AVAuthorizationStatus =
            msg_send![av_capture_device, authorizationStatusForMediaType:media_type_audio];

        match status {
            AVAuthorizationStatus::NotDetermined => PermissionState::NotRequested,
            AVAuthorizationStatus::Restricted | AVAuthorizationStatus::Denied => {
                PermissionState::Denied
            }
            AVAuthorizationStatus::Authorized => PermissionState::Granted,
        }
    }
}

pub fn check_accessibility_permission() -> PermissionState {
    unsafe {
        let trusted: BOOL = ax_is_process_trusted();
        if trusted == YES {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        }
    }
}

pub fn request_permission(permission_type: &str) -> Result<(), String> {
    match permission_type {
        "microphone" => request_microphone_permission(),
        "accessibility" => request_accessibility_permission(),
        _ => Err(format!("Unknown permission type: {}", permission_type)),
    }
}

fn request_microphone_permission() -> Result<(), String> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let av_capture_device = class!(AVCaptureDevice);
        let media_type_audio = NSString::alloc(nil).init_str("soun");

        let _: () = msg_send![av_capture_device, requestAccessForMediaType:media_type_audio completionHandler: nil];

        Ok(())
    }
}

fn request_accessibility_permission() -> Result<(), String> {
    unsafe {
        let options: core_foundation::dictionary::CFDictionary<
            core_foundation::string::CFString,
            core_foundation::base::CFType,
        > = core_foundation::dictionary::CFDictionary::from_CFType_pairs(&[]);
        ax_is_process_trusted_with_options(options.as_concrete_TypeRef());
        Ok(())
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> BOOL;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef)
        -> BOOL;
}

unsafe fn ax_is_process_trusted() -> BOOL {
    AXIsProcessTrusted()
}

unsafe fn ax_is_process_trusted_with_options(
    options: core_foundation::dictionary::CFDictionaryRef,
) -> BOOL {
    AXIsProcessTrustedWithOptions(options)
}
