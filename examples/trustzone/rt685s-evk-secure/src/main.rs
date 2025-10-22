#![no_std]
#![no_main]
#![feature(abi_cmse_nonsecure_call)]
#![feature(cmse_nonsecure_entry)]

use core::ops::Range;
use core::panic::PanicInfo;
use core::sync::atomic::AtomicU32;
use core::u32;

use cortex_m::peripheral::sau::SauRegion;
use cortex_m::peripheral::{NVIC, SCB};
use mimxrt685s_pac::ahb_secure_ctrl::ram00_rule::Rule0;
use mimxrt685s_pac::{ahb_secure_ctrl, interrupt, AhbSecureCtrl, Interrupt, ScnScb};
use rtt_target::rprintln;
use rtt_target::ChannelMode::NoBlockSkip;

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0; 256];

#[link_section = ".keystore"]
#[used]
static KEYSTORE: [u8; 2048] = [0; 2048];

const SECURE_START_FLASH: u32 = 0x10170000;
const NONSECURE_START_FLASH: *const [u32; 2] = 0x08006000 as *const [u32; 2];
const SECURE_START_RAM: u32 = 0x20000000;
const NONSECURE_START_RAM: u32 = 0x20001000;
const NONSECURE_START_PERIPHERALS: u32 = 0x40000000;

extern "Rust" {
    static __veneer_base: ();
    static __veneer_limit: ();
}

const VTOR_S: *mut u32 = 0xE002ED08 as *mut u32;
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

        let ahb_secure_ctrl = &*AhbSecureCtrl::ptr().add(0x1000_0000);

        // rprintln!("Configuring secure_violation_irq");
        ahb_secure_ctrl.misc_ctrl_reg().modify(|_, w| {
            w.disable_violation_abort()
                .enable()
                .idau_all_ns()
                .disable()
                .enable_ns_priv_check()
                .disable()
                .enable_s_priv_check()
                .disable()
            // .disable_simple_master_strict_mode()
            // .tier_mode()
            // .disable_smart_master_strict_mode()
            // .tier_mode()
        });
        ahb_secure_ctrl.misc_ctrl_dp_reg().modify(|_, w| {
            w.disable_violation_abort()
                .enable()
                .idau_all_ns()
                .disable()
                .enable_ns_priv_check()
                .disable()
                .enable_s_priv_check()
                .disable()
            // .disable_simple_master_strict_mode()
            // .tier_mode()
            // .disable_smart_master_strict_mode()
            // .tier_mode()
        });

        rprintln!("Setting up regions");
        rprintln!(
            "Veneers: {:#010X} .. {:#010X}",
            &raw const __veneer_base as u32,
            &raw const __veneer_limit as u32
        );

        NVIC::unmask(Interrupt::SECUREVIOLATION);

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
                    base_address: &raw const __veneer_base as u32,
                    limit_address: &raw const __veneer_limit as u32 - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecureCallable,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                1,
                SauRegion {
                    base_address: 0x10170000,
                    limit_address: 0x10176000 - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::Secure,
                },
            )
            .unwrap();
        cp.SAU
            .set_region(
                3,
                SauRegion {
                    base_address: 0x2100_0000,
                    limit_address: 0x2200_0000 - 1,
                    attribute: cortex_m::peripheral::sau::SauRegionAttribute::NonSecureCallable,
                },
            )
            .unwrap();

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        cp.SAU.enable();

        // Then set the ROM to secure only
        rprintln!("Set ROM to secure");
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
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        rprintln!("Set ROM range to secure");
        set_ram_secure(0x2014_0000..0x2017_0000, ahb_secure_ctrl);
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        rprintln!("Set secure RAM for code and data to secure");
        set_ram_secure(0x2017_0000..0x2018_0000, ahb_secure_ctrl);
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        rprintln!("Setting all bus masters to non-secure");
        ahb_secure_ctrl.master_sec_level().modify(|_, w| {
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
        ahb_secure_ctrl.master_sec_level_anti_pol().modify(|_, w| {
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

        rprintln!(
            "misc_ctrl_reg: {:#010X}, {:#010X}",
            ahb_secure_ctrl.misc_ctrl_reg().read().bits(),
            ahb_secure_ctrl.misc_ctrl_dp_reg().read().bits()
        );

        rtt_target::UpChannel::conjure(0).unwrap().flush();

        // Make sure the new settings take effect immediately:
        // https://developer.arm.com/documentation/100235/0100/The-Cortex-M33-Peripherals/Security-Attribution-and--Memory-Protection/Updating-protected-memory-regions
        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        // Set the nonsecure VTOR
        // VTOR_NS.write_volatile(NONSECURE_START_FLASH as u32);
        VTOR_NS.write_volatile(SECURE_START_FLASH as u32);
        cp.SCB.vtor.write(SECURE_START_FLASH as u32);

        // Set all interrupts to non-secure
        for itns in &cp.NVIC.itns {
            itns.write(u32::MAX);
            // itns.write(0);
        }

        // Set the non-secure stack pointer
        // cortex_m::register::msp::write_ns(nonsecure_sp);

        rprintln!("Jumping");
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        //[NSACR_CP0, NSACR_CP1, NSACR_CP10, NSACR_CP11, AHB_MISC_CTRL_REG_ENABLE_SECURE_CHECKING, AHB_MISC_CTRL_REG_WRITE_LOCK]

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        ahb_secure_ctrl.apb_grp0_mem_rule0().write(|w| w.bits(0xFCFFFFFF));
        ahb_secure_ctrl.apb_grp0_mem_rule1().write(|w| w.bits(0xCCFFFFFF));
        ahb_secure_ctrl.apb_grp1_mem_rule0().write(|w| w.bits(0xFCCFFFFF));
        ahb_secure_ctrl.apb_grp1_mem_rule1().write(|w| w.bits(0xCCCCCCCC));
        ahb_secure_ctrl.apb_grp1_mem_rule2().write(|w| w.bits(0xFCFFFFFC));

        ahb_secure_ctrl.ahb_periph0_slave_rule0().write(|w| w.bits(0xFCCCFCCC));
        ahb_secure_ctrl.aips_bridge0_mem_rule0().write(|w| w.bits(0xFFFCCCCC));
        ahb_secure_ctrl.ahb_periph1_slave_rule0().write(|w| w.bits(0xCCCCCCCC));
        ahb_secure_ctrl.aips_bridge1_mem_rule0().write(|w| w.bits(0xCCFCFFFF));
        ahb_secure_ctrl.aips_bridge1_mem_rule1().write(|w| w.bits(0xFFFFCCCC));
        ahb_secure_ctrl.ahb_periph2_slave_rule0().write(|w| w.bits(0xFFFFCCCF));
        ahb_secure_ctrl.ahb_periph3_slave_rule0().write(|w| w.bits(0xFFFCCFCC));

        let scn_scb = &*ScnScb::ptr().add(0x1000_0000);
        scn_scb.cppwr().modify(|_, w| w.bits(0));

        // let scb = &*SCB::ptr();
        // scb.shcsr.modify(|w| w & 0x0FFF7FFFF); // Disable bit 19 / SECUREFAULTENA
        cp.SCB.shcsr.modify(|w| w | (1 << 19));

        let ptr = __cortex_m_rt_HardFault_trampoline as *mut u32;
        rprintln!(
            "TT {:#010X}, {:#010X}, {:#010X}, {:#010X}, {:#010X}",
            ptr as u32,
            cortex_m::asm::tt(ptr),
            cortex_m::asm::ttt(ptr),
            cortex_m::asm::tta(ptr),
            cortex_m::asm::ttat(ptr),
        );
        let ptr = 0x21000000 as *mut u32;
        rprintln!(
            "TT {:#010X}, {:#010X}, {:#010X}, {:#010X}, {:#010X}",
            ptr as u32,
            cortex_m::asm::tt(ptr),
            cortex_m::asm::ttt(ptr),
            cortex_m::asm::tta(ptr),
            cortex_m::asm::ttat(ptr),
        );

        rprintln!("Locking master sec level");
        ahb_secure_ctrl
            .master_sec_level()
            .modify(|_, w| w.master_sec_level_lock().blocked());
        ahb_secure_ctrl
            .master_sec_level_anti_pol()
            .modify(|_, w| w.master_sec_level_anti_pole_lock().blocked());

        rprintln!(
            "master_sec_level: {:#010X}, {:#010X}",
            ahb_secure_ctrl.master_sec_level().read().bits(),
            ahb_secure_ctrl.master_sec_level_anti_pol().read().bits()
        );

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        // rprintln!("Test");
        // __cortex_m_rt_HardFault();

        rprintln!("Enabling AHB bus checks (1/2)");
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        ahb_secure_ctrl
            .misc_ctrl_reg()
            .modify(|_, w| w.enable_secure_checking().enable());

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        rprintln!("Enabling AHB bus checks (2/2)");
        rtt_target::UpChannel::conjure(0).unwrap().flush();

        ahb_secure_ctrl
            .misc_ctrl_dp_reg()
            .modify(|_, w| w.enable_secure_checking().enable());

        rprintln!(
            "misc_ctrl_reg: {:#010X}, {:#010X}",
            ahb_secure_ctrl.misc_ctrl_reg().read().bits(),
            ahb_secure_ctrl.misc_ctrl_dp_reg().read().bits()
        );

        // Create the right function pointer to the reset vector
        let nonsecure_reset =
            core::mem::transmute::<*const u32, extern "cmse-nonsecure-call" fn()>(nonsecure_reset as *const u32);

        // Enable the secure fault
        cp.SCB.shcsr.modify(|w| w | (1 << 19));
        // Prioritize and allow faults in the non-secure side
        // cp.SCB.aircr.modify(|w| w & !(1 << 14) & !(1 << 13));

        loop {
            cortex_m::asm::nop();
        }

        // Jump
        // nonsecure_reset();

        // cortex_m::asm::udf();
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
    let scb = &*cortex_m::peripheral::SCB::PTR;
    rprintln!("HardFault! - HFSR: {:#010X}", scb.hfsr.read());
    loop {
        cortex_m::asm::nop();
    }
}

#[interrupt]
unsafe fn SECUREVIOLATION() {
    rprintln!("SecureViolation!");
    loop {
        cortex_m::asm::nop();
    }
}

#[panic_handler]
fn panic(i: &PanicInfo) -> ! {
    rprintln!("{}", i);
    loop {
        cortex_m::asm::nop();
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
extern "cmse-nonsecure-entry" fn do_stuff_secure(num: u32) -> u32 {
    let old = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    num * 2 + old
}
