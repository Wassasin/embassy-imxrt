#![no_std]
#![no_main]
#![feature(abi_cmse_nonsecure_call)]
#![feature(cmse_nonsecure_entry)]

use core::ops::Range;
use core::panic::PanicInfo;
use core::sync::atomic::AtomicU32;
use core::u32;

use cortex_m::peripheral::sau::SauRegion;
use mimxrt600_fcb::FlexSPIFlashConfigurationBlock;
use mimxrt685s_pac::ahb_secure_ctrl::ram00_rule::Rule0;
use mimxrt685s_pac::{ahb_secure_ctrl, AhbSecureCtrl};
use rtt_target::rprintln;
use rtt_target::ChannelMode::NoBlockSkip;

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
const SECURE_START_RAM: u32 = 0x20000000;
const NONSECURE_START_RAM: u32 = 0x20001000;

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
    let _dp = mimxrt685s_pac::Peripherals::take().unwrap();

    unsafe {
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

        // Set all regions not used by this program to non-secure.
        // This only concerns the CM33 access, not any of the other bus masters like DMA
        rprintln!("Set SAU");
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
        rprintln!("Set ROM to secure");
        let ahb_secure_ctrl = &*AhbSecureCtrl::ptr().add(0x1000_0000);
        for rom_mem in ahb_secure_ctrl.rom_mem_rule_iter() {
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
        // Set the secure RAM to secure only
        rprintln!("Set RAM to secure");
        set_ram_secure(SECURE_START_RAM..NONSECURE_START_RAM, ahb_secure_ctrl);

        rprintln!("Setting all bus masters to non-secure");
        ahb_secure_ctrl.master_sec_level().write(|w| {
            w.dma0_sec()
                .enum_ns_np()
                .dma1_sec()
                .enum_ns_np()
                .dsp_sec()
                .enum_ns_np()
                .powerquad_sec()
                .enum_ns_np()
                .sdio0_sec()
                .enum_ns_np()
                .sdio1_sec()
                .enum_ns_np()
                .master_sec_level_lock()
                .writable()
        });
        ahb_secure_ctrl.master_sec_level_anti_pol().write(|w| {
            w.dma0_sec()
                .enum_ns_np()
                .dma1_sec()
                .enum_ns_np()
                .dsp_sec()
                .enum_s_p()
                .powerquad_sec()
                .enum_ns_np()
                .sdio0_sec()
                .enum_ns_np()
                .sdio1_sec()
                .enum_ns_np()
                .master_sec_level_anti_pole_lock()
                .writable()
        });
        rprintln!(
            "master_sec_level: {:#010X}, {:#010X}",
            ahb_secure_ctrl.master_sec_level().read().bits(),
            ahb_secure_ctrl.master_sec_level_anti_pol().read().bits()
        );

        rprintln!("Enabling AHB bus checks");
        ahb_secure_ctrl.misc_ctrl_reg().write(|w| {
            w.enable_ns_priv_check()
                .enable()
                .enable_secure_checking()
                .enable()
                .enable_s_priv_check()
                .enable()
        });
        ahb_secure_ctrl.misc_ctrl_dp_reg().write(|w| {
            w.enable_ns_priv_check()
                .enable()
                .enable_secure_checking()
                .enable()
                .enable_s_priv_check()
                .enable()
        });

        rprintln!(
            "misc_ctrl_reg: {:#010X}, {:#010X}",
            ahb_secure_ctrl.misc_ctrl_reg().read().bits(),
            ahb_secure_ctrl.misc_ctrl_dp_reg().read().bits()
        );

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

        // Enable the secure fault
        cp.SCB.shcsr.modify(|w| w | (1 << 19));
        // Prioritize and allow faults in the non-secure side
        // cp.SCB.aircr.modify(|w| w & !(1 << 14) & !(1 << 13));

        // Jump
        nonsecure_reset();

        cortex_m::asm::udf();
    }
}

fn set_ram_secure(mut region: Range<u32>, ahb_secure_ctrl: &ahb_secure_ctrl::RegisterBlock) {
    let address_to_block = |address: u32| {
        const BLOCK_SIZE_TABLE: &[(u32, u32, u32)] = &[
            (
                0x2010_0000,
                8192,
                0x4_0000 / 0x400 + 0x4_0000 / 0x800 + 0x8_0000 / 0x1000,
            ),
            (0x2008_0000, 4096, 0x4_0000 / 0x400 + 0x4_0000 / 0x800),
            (0x2004_0000, 2048, 0x4_0000 / 0x400),
            (0x2000_0000, 1024, 0),
        ];

        let (block_start, block_size, previous_blocks) = BLOCK_SIZE_TABLE
            .iter()
            .find(|(block_address, _, _)| address >= *block_address)
            .unwrap();

        (
            *previous_blocks + (address - *block_start) / *block_size,
            *block_start + (address - *block_start) / *block_size * *block_size,
            *block_size,
        )
    };

    // Make sure the end of the range is at a boundary
    let (_, block_start, _) = address_to_block(region.end);
    assert_eq!(region.end, block_start);

    while !region.is_empty() {
        let (block_index, block_start, block_size) = address_to_block(region.start);
        assert_eq!(region.start, block_start);

        let register_index = block_index / 8;
        let rule_index = block_index % 8;

        let base_ptr = ahb_secure_ctrl.ram00_rule(0).as_ptr();

        rprintln!(
            "Ram region {:#010X}..{:#010X} to secure ({}, {})",
            block_start,
            block_start + block_size - 1,
            register_index,
            rule_index
        );

        unsafe {
            let target_register = base_ptr.add(register_index as usize);
            let current_val = target_register.read_volatile();
            target_register.write_volatile(
                current_val & !(0b11 << (rule_index * 4)) | ((Rule0::SecurePrivUserAllowed as u32) << (rule_index * 4)),
            );
        }

        region.start += block_size;
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
    rprintln!("{}", i);
    cortex_m::asm::udf();
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
extern "cmse-nonsecure-entry" fn do_stuff_secure(num: u32) -> u32 {
    let old = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    num * 2 + old
}
