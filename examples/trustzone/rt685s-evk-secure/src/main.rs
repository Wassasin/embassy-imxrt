#![no_std]
#![no_main]
#![feature(abi_cmse_nonsecure_call)]
#![feature(cmse_nonsecure_entry)]

use core::ops::Range;
use core::panic::PanicInfo;
use core::sync::atomic::{fence, AtomicU32, Ordering};
use core::u32;

use cortex_m::peripheral::sau::SauRegion;
use cortex_m::peripheral::SCB;
use mimxrt600_fcb::FlexSPIFlashConfigurationBlock;
use mimxrt685s_pac::ahb_secure_ctrl::ram00_rule::Rule0;
use mimxrt685s_pac::{ahb_secure_ctrl, interrupt, AhbSecureCtrl, Interrupt, Sau, ScnScb};
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
const NONSECURE_START_FLASH: *const [u32; 2] = 0x08040000 as *const [u32; 2];
const SECURE_START_RAM: u32 = 0x20000000;
const NONSECURE_START_RAM: u32 = 0x20001000;
const NONSECURE_END_RAM: u32 = 0x2FFFFFFF;

extern "Rust" {
    static __veneer_base: ();
    static __veneer_limit: ();
}

const VTOR_NS: *mut u32 = 0xE002ED08 as *mut u32;

fn test_security(addr: *mut u32) {
    fn opt_if<T>(val: bool, some: T) -> Option<T> {
        if val {
            Some(some)
        } else {
            None
        }
    }

    let res = cortex_m::asm::ttat(addr);
    let mrvalid = res >> 16 & 0b1 == 0b1;
    let srvalid = res >> 17 & 0b1 == 0b1;
    let irvalid = res >> 23 & 0b1 == 0b1;
    let mregion = opt_if(mrvalid, res & 0xf);
    let sregion = opt_if(srvalid, res >> 8 & 0xf);
    let iregion = opt_if(irvalid, res >> 24 & 0xf);
    let r = res >> 18 & 0b1 == 0b1;
    let rw = res >> 19 & 0b1 == 0b1;
    let s = res >> 22 & 0b1 == 0b1;
    rprintln!(
        "ttat({:#010X}): {:#010X} (mregion: {:?}, sregion {:?}, iregion {:?}, r {}, rw {}, s {})",
        addr as u32,
        res,
        mregion,
        sregion,
        iregion,
        r,
        rw,
        s
    );
}

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
        let sau = &*Sau::ptr();

        sau.ctrl().write(|w| w.enable().disabled());

        cp.SAU
            .set_region(
                0,
                SauRegion {
                    base_address: 0,
                    limit_address: SECURE_START_FLASH - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::Secure,
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
                    limit_address: 0x08FFFFFF,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                3,
                SauRegion {
                    base_address: NONSECURE_START_RAM,
                    limit_address: NONSECURE_END_RAM,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                4,
                SauRegion {
                    base_address: 0x4000_0000,
                    limit_address: 0x4FFF_FFFF,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecure,
                },
            )
            .unwrap();

        // Then set the ROM to secure only
        // rprintln!("Set ROM to secure");
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

        // Set FlexSPI settings
        ahb_secure_ctrl.flexspi0_region0_rule(0).write(|w| w.bits(0x00000003));
        ahb_secure_ctrl.flexspi0_region0_rule(1).write(|w| w.bits(0x00000000));
        ahb_secure_ctrl.flexspi0_region0_rule(2).write(|w| w.bits(0x00000000));
        ahb_secure_ctrl.flexspi0_region0_rule(3).write(|w| w.bits(0x00000000));

        ahb_secure_ctrl.flexspi0_region1_rule0().write(|w| w.bits(0x00000000));
        ahb_secure_ctrl.flexspi0_region2_rule0().write(|w| w.bits(0x00000000));
        ahb_secure_ctrl.flexspi0_region3_rule0().write(|w| w.bits(0x00000000));
        ahb_secure_ctrl.flexspi0_region4_rule0().write(|w| w.bits(0x00000000));

        ahb_secure_ctrl.ahb_periph2_slave_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.pif_hifi4_x_mem_rule0().write(|w| w.bits(0));

        // Set all peripherals to NonSecure
        ahb_secure_ctrl.apb_grp0_mem_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.apb_grp0_mem_rule1().write(|w| w.bits(0));
        ahb_secure_ctrl.apb_grp1_mem_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.apb_grp1_mem_rule1().write(|w| w.bits(0));
        ahb_secure_ctrl.apb_grp1_mem_rule2().write(|w| w.bits(0));
        ahb_secure_ctrl.ahb_periph0_slave_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.aips_bridge0_mem_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.ahb_periph1_slave_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.aips_bridge1_mem_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.aips_bridge1_mem_rule1().write(|w| w.bits(0));
        ahb_secure_ctrl.ahb_periph2_slave_rule0().write(|w| w.bits(0));
        ahb_secure_ctrl.ahb_periph3_slave_rule0().write(|w| w.bits(0));

        rprintln!("Setting all bus masters to non-secure");
        ahb_secure_ctrl.master_sec_level().write(|w| w.bits(0x80000000));
        ahb_secure_ctrl
            .master_sec_level_anti_pol()
            .write(|w| w.bits(0xBFFFFFFF));
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
                .enum_ns_np()
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

        cp.SCB.aircr.write((cp.SCB.aircr.read() & 0x000009FF7) | 0x005FA0000);
        cp.SCB.scr.modify(|w| w & 0x0FFFFFFF7);
        cp.SCB.shcsr.modify(|w| w & 0x0FFF7FFFF);

        // // SCB->NSACR                        = 0x00000C03U;
        let nsacr = 0xE000ED8C as *mut u32;
        nsacr.write_volatile(0x00000C03);

        // // SCnSCB->CPPWR                     = 0;
        let scn_scb = &*ScnScb::ptr().add(0x1000_0000);
        scn_scb.cppwr().write(|w| w.bits(0));

        // ahb_secure_ctrl
        //     .sec_mask_lock()
        //     .write(|w| w.bits((ahb_secure_ctrl.sec_mask_lock().read().bits() & 0x0FFFCFFC0) | 0x00002002A));

        // ahb_secure_ctrl
        //     .master_sec_level()
        //     .write(|w| w.bits((ahb_secure_ctrl.master_sec_level().read().bits() & 0x03FFFFFFF) | 0x080000000));
        // ahb_secure_ctrl
        //     .master_sec_level_anti_pol()
        //     .write(|w| w.bits((ahb_secure_ctrl.master_sec_level_anti_pol().read().bits() & 0x03FFFFFFF) | 0x080000000));

        // ahb_secure_ctrl.cm33_lock_reg().write(|w| w.bits(0x800002AA));

        // fence(Ordering::SeqCst);

        rprintln!("Enabling AHB bus checks");
        ahb_secure_ctrl.misc_ctrl_reg().modify(|_, w| {
            w.enable_ns_priv_check()
                .disable()
                .enable_secure_checking()
                .disable()
                .enable_s_priv_check()
                .disable()
            // .idau_all_ns()
            // .enable()
            // .disable_violation_abort()
            // .disable()
            // .disable_simple_master_strict_mode()
            // .tier_mode()
            // .disable_smart_master_strict_mode()
            // .tier_mode()
        });
        ahb_secure_ctrl.misc_ctrl_dp_reg().modify(|_, w| {
            w.enable_ns_priv_check()
                .disable()
                .enable_secure_checking()
                .disable()
                .enable_s_priv_check()
                .disable()
            // .idau_all_ns()
            // .enable()
            // .disable_violation_abort()
            // .disable()
            // .disable_simple_master_strict_mode()
            // .tier_mode()
            // .disable_smart_master_strict_mode()
            // .tier_mode()
        });

        // Enable it all.
        ahb_secure_ctrl
            .misc_ctrl_reg()
            .modify(|_, w| w.enable_secure_checking().enable());
        ahb_secure_ctrl
            .misc_ctrl_dp_reg()
            .modify(|_, w| w.enable_secure_checking().enable());

        // ahb_secure_ctrl.misc_ctrl_reg().write(|w| {
        //     w.bits(0x0000AAA5);
        //     // w.enable_secure_checking().disable();
        //     w
        // });
        // ahb_secure_ctrl.misc_ctrl_dp_reg().write(|w| {
        //     w.bits(0x0000AAA5);
        //     // w.enable_secure_checking().disable();
        //     w
        // });

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

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

        cp.SAU.enable();

        // Enable the secure fault
        cp.SCB.shcsr.modify(|w| w | (1 << 19 | 1 << 18 | 1 << 17));

        cortex_m::peripheral::NVIC::unmask(Interrupt::SECUREVIOLATION);

        rprintln!("Jumping");

        test_security(&raw const __veneer_base as u32 as *mut u32);
        test_security(nonsecure_reset as *mut u32);

        // Create the right function pointer to the reset vector
        let nonsecure_reset =
            core::mem::transmute::<*const u32, extern "cmse-nonsecure-call" fn()>(nonsecure_reset as *const u32);

        // Prioritize and allow faults in the non-secure side
        // cp.SCB.aircr.modify(|w| w & !(1 << 14) & !(1 << 13));

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        // Jump
        nonsecure_reset();

        cortex_m::asm::udf();

        // loop {
        //     cortex_m::asm::nop();
        // }
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
    let scb = &*SCB::PTR;
    rprintln!("HardFault! (S) - SHCSR: {:#010X}", scb.shcsr.read());

    loop {
        cortex_m::asm::nop();
    }
}

#[cortex_m_rt::exception]
unsafe fn UsageFault() -> ! {
    rprintln!("UsageFault!");
    loop {
        cortex_m::asm::nop();
    }
}

#[cortex_m_rt::exception]
unsafe fn BusFault() -> ! {
    let scb = &*SCB::PTR;
    rprintln!(
        "BusFault! - CFSR: {:#010X}, BFAR: {:#010X}",
        scb.cfsr.read(),
        scb.bfar.read()
    );
    loop {
        cortex_m::asm::nop();
    }
}

#[interrupt]
unsafe fn SECUREVIOLATION() {
    rprintln!("SECUREVIOLATION!");
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
