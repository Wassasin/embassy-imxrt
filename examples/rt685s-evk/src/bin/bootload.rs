#![no_std]
#![no_main]

use defmt::{info, Format};
use embassy_executor::Spawner;
use embassy_imxrt::interrupt;
use embassy_time::Timer;
use {defmt_rtt as _, embassy_imxrt_examples as _, panic_probe as _};

// bind_interrupts!(struct Irqs {
//     HASHCRYPT => InterruptHandler;
// });

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_imxrt::init(Default::default());

    info!("Hello bootloader");

    let addresses = [
        0x08000000 as *const u32,
        0x08000400 as *const u32,
        0x08000600 as *const u32,
        0x08000800 as *const u32,
        0x08001000 as *const u32,
    ];

    for ptr in addresses {
        let verified = skboot_authenticate(ptr);
        info!("Result {:x} {:?}", ptr, verified);
    }

    loop {
        Timer::after_millis(1000).await;
    }
}

#[interrupt]
fn HASHCRYPT() {
    info!("hashcrypt interrupt")
}

// pub struct InterruptHandler {
//     _phantom: (),
// }

// impl interrupt::typelevel::Handler<interrupt::typelevel::HASHCRYPT> for InterruptHandler {
//     unsafe fn on_interrupt() {
//         info!("hashcrypt interrupt")
//     }
// }

// skboot_authenticate_api
// skboot_status_t skboot_authenticate_api(const uint8_t *imageStartAddr, secure_bool_t *isSignVerified);
// static SKBOOT_AUTHENTICATE: AuthenticateFun = unsafe { core::mem::transmute(0x1300DFC7 as *const ()) };

enum BootStatus {
    Success,
    Fail,
    InvalidArgument,
    KeyStoreMarkerInvalid,
    HashcryptFinishedWithStatusSuccess,
    HashcryptFinishedWithStatusFail,
}

impl TryFrom<u32> for BootStatus {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0x5ac3c35a => BootStatus::Success,
            0xc35ac35a => BootStatus::Fail,
            0xc35a5ac3 => BootStatus::InvalidArgument,
            0xc3c35a5a => BootStatus::KeyStoreMarkerInvalid,
            0xc15a5ac3 => BootStatus::HashcryptFinishedWithStatusSuccess,
            0xc15a5acb => BootStatus::HashcryptFinishedWithStatusFail,
            _ => return Err(()),
        })
    }
}

enum SecureBool {
    True,
    False,
    CallProtectSecurityFlags,
    CallProtectIsAppReady,
    TrackerVerified,
}

impl TryFrom<u32> for SecureBool {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0xc33cc33c => SecureBool::True,
            0x5aa55aa5 => SecureBool::False,
            0xc33c5aa5 => SecureBool::CallProtectSecurityFlags,
            0x5aa5c33c => SecureBool::CallProtectIsAppReady,
            0x55aacc33 => SecureBool::TrackerVerified,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Format)]
pub enum AuthenticateError {
    /// Failed to verify signature.
    SignUnverified,
    /// Failed to verify signature with unknown error.
    SignUnknown,
    /// Failed to authenticate image when parsing certificate header, certificate chain RKH or signature verification fails.
    Fail,
    /// Found an unexpected value in image.
    UnexpectedValueInImage,
    /// The keystore marker on the image is invalid.
    KeyStoreMarkerInvalid,
    /// The function passed an undefined return value.
    BootStatusUnknown,
    /// The function passed an undefined value as `is_sign_verified`` value.
    IsSignVerifiedUnknown,
}

fn skboot_authenticate(start: *const u32) -> Result<(), AuthenticateError> {
    type AuthenticateFun = fn(*const u32, *mut u32) -> u32;

    // Note:
    // The ROM reserved space for global variables in RAM on this device is:
    // 0x1001_2000 to 0x1000_A000

    // 43.9 Secure ROM API page 1282 of RT6xx User manual
    // let funptr = 0x1300_DFC7 as *const ();
    let funptr = 0x0300_DFC7 as *const ();
    let funptr: AuthenticateFun = unsafe { core::mem::transmute(funptr) };

    let mut is_sign_verified: u32 = 0;

    let result = funptr(start, &mut is_sign_verified);

    match BootStatus::try_from(result).map_err(|()| AuthenticateError::BootStatusUnknown)? {
        BootStatus::Success => {
            match SecureBool::try_from(is_sign_verified).map_err(|()| AuthenticateError::IsSignVerifiedUnknown)? {
                SecureBool::TrackerVerified => Ok(()),
                SecureBool::False => Err(AuthenticateError::SignUnverified),
                _ => Err(AuthenticateError::SignUnknown),
            }
        }
        BootStatus::Fail => Err(AuthenticateError::Fail),
        BootStatus::InvalidArgument => Err(AuthenticateError::UnexpectedValueInImage),
        BootStatus::KeyStoreMarkerInvalid => Err(AuthenticateError::KeyStoreMarkerInvalid),
        _ => Err(AuthenticateError::BootStatusUnknown),
    }
}
