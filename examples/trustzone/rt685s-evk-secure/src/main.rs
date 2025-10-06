#![no_std]
#![no_main]
#![feature(abi_cmse_nonsecure_call)]
#![feature(cmse_nonsecure_entry)]

use core::panic::PanicInfo;
use core::sync::atomic::AtomicU32;
use core::u32;

use cortex_m::peripheral::sau::SauRegion;
use mimxrt600_fcb::FlexSPIFlashConfigurationBlock;
use rtt_target::ChannelMode::NoBlockSkip;
use rtt_target::{debug_rprintln, rprintln};

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0; 256];

#[rustfmt::skip]
#[link_section = ".fcb"]
#[used]
static FCB: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build();

#[link_section = ".keystore"]
#[used]
static KEYSTORE: [u8; 2048] = [0; 2048];

const SECURE_START_FLASH: u32 = 0x08001000;
const NONSECURE_START_FLASH: *const [u32; 2] = 0x08006000 as *const [u32; 2];
const SECURE_START_RAM: u32 = 0x20080000;
const NONSECURE_START_RAM: u32 = 0x20081000;

extern "Rust" {
    static __veneer_base: ();
    static __veneer_limit: ();
}

const VTOR_NS: *mut u32 = 0xE002ED08 as *mut u32;

#[cortex_m_rt::entry]
fn main() -> ! {
    let channels = rtt_target::rtt_init! {
        up: {
            0: { // channel number
                size: 1024, // buffer size in bytes
                mode: NoBlockSkip, // mode (optional, default: NoBlockSkip, see enum ChannelMode)
                name: "Terminal", // name (optional, default: no name)
                section: ".shared_rtt.buffer" // Buffer linker section (optional, default: no section)
            }
        }
        section_cb: ".shared_rtt.header" // Control block linker section (optional, default: no section)
    };
    rtt_target::set_print_channel(channels.up.0);

    let mut cp = cortex_m::Peripherals::take().unwrap();
    let dp = mimxrt685s_pac::Peripherals::take().unwrap();

    unsafe {
        // Enable the secure fault
        cp.SCB.shcsr.modify(|w| w | (1 << 19));

        let [nonsecure_sp, nonsecure_reset] = NONSECURE_START_FLASH.read_volatile();

        rprintln!("Running. SP: {:#010X}, RV: {:#010X}", nonsecure_sp, nonsecure_reset);

        if nonsecure_sp == u32::MAX || nonsecure_reset == u32::MAX {
            loop {
                cortex_m::asm::nop();
            }
        }

        rprintln!("Setting up regions");
        rprintln!(
            "Veneers: {:#010X} .. {:#010X}",
            &raw const __veneer_base as u32,
            &raw const __veneer_limit as u32
        );

        // Make sure all writes and reads are done before changing the security settings
        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        // Set all regions not used by this program to non-secure
        cp.SAU
            .set_region(
                0,
                SauRegion {
                    base_address: 0,
                    limit_address: SECURE_START_FLASH - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                1,
                SauRegion {
                    base_address: &raw const __veneer_base as u32,
                    limit_address: &raw const __veneer_limit as u32 - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecureCallable,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                2,
                SauRegion {
                    base_address: NONSECURE_START_FLASH as u32,
                    limit_address: SECURE_START_RAM - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                3,
                SauRegion {
                    base_address: NONSECURE_START_RAM,
                    limit_address: u32::MAX,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();
        cp.SAU.enable();

        // Then set the ROM to secure only
        // The nonsecure code can only control nonsecure peripherals like DMA
        for rom_mem in dp.ahb_secure_ctrl.rom_mem_rule_iter() {
            rom_mem.write(|w| {
                w.rule0().secure_nonpriv_user_allowed();
                w.rule1().secure_nonpriv_user_allowed();
                w.rule2().secure_nonpriv_user_allowed();
                w.rule3().secure_nonpriv_user_allowed();
                w.rule4().secure_nonpriv_user_allowed();
                w.rule5().secure_nonpriv_user_allowed();
                w.rule6().secure_nonpriv_user_allowed();
                w.rule7().secure_nonpriv_user_allowed()
            });
        }

        // Make sure the new settings take effect immediately:
        // https://developer.arm.com/documentation/100235/0100/The-Cortex-M33-Peripherals/Security-Attribution-and--Memory-Protection/Updating-protected-memory-regions
        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        // Set the nonsecure VTOR
        VTOR_NS.write_volatile(NONSECURE_START_FLASH as u32);

        // Set all interrupts to non-secure
        for itns in &cp.NVIC.itns {
            itns.write(u32::MAX);
        }

        // Set the non-secure stack pointer
        cortex_m::register::msp::write_ns(nonsecure_sp);

        rprintln!("Jumping");

        // Create the right function pointer to the reset vector
        let nonsecure_reset =
            core::mem::transmute::<*const u32, extern "cmse-nonsecure-call" fn()>(nonsecure_reset as *const u32);
        // Jump
        nonsecure_reset();

        cortex_m::asm::udf();
    }
}

#[cortex_m_rt::exception]
unsafe fn SecureFault() -> ! {
    let sau = &*cortex_m::peripheral::SAU::PTR;
    rprintln!(
        "SecureFault! - SFSR: {:#010X}, SFAR: {:#010X}",
        sau.sfsr.read().0,
        sau.sfar.read().0
    );
    loop {
        cortex_m::asm::nop();
    }
}

#[cortex_m_rt::exception(trampoline = false)]
unsafe fn HardFault() -> ! {
    rprintln!("HardFault!");
    loop {
        cortex_m::asm::nop();
    }
}

#[panic_handler]
fn panic(i: &PanicInfo) -> ! {
    debug_rprintln!("{}", i);
    cortex_m::asm::udf();
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
extern "cmse-nonsecure-entry" fn do_stuff_secure(num: u32) -> u32 {
    let old = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    num * 2 + old
}
