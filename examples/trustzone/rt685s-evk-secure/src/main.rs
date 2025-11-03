#![no_std]
#![no_main]
#![feature(abi_cmse_nonsecure_call)]
#![feature(cmse_nonsecure_entry)]

use core::ops::Range;
use core::panic::PanicInfo;
use core::sync::atomic::{compiler_fence, AtomicU32};
use core::u32;

use cortex_m::peripheral::sau::SauRegion;
use cortex_m::peripheral::{NVIC, SCB};
use mimxrt685s_pac::ahb_secure_ctrl::ram00_rule::Rule0;
use mimxrt685s_pac::{ahb_secure_ctrl, interrupt, AhbSecureCtrl, Clkctl0, Clkctl1, Gpio, Interrupt, Rstctl1, ScnScb};
// use // rtt_target::rprintln;
// use // rtt_target::ChannelMode::NoBlockSkip;

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
    let cp = cortex_m::Peripherals::take().unwrap();
    let _dp = mimxrt685s_pac::Peripherals::take().unwrap();

    unsafe {
        let clkctl1 = &*Clkctl1::ptr();
        clkctl1.pscctl1_set().write(|w| w.hsgpio0_clk_set().set_bit());

        let rstctl1 = &*Rstctl1::ptr();
        rstctl1.prstctl1_clr().write(|w| w.hsgpio0_rst_clr().set_bit());

        let gpio = &*Gpio::ptr();
        // gpio.set(0).write(|w| w.setp().bits(1 << 26));
        gpio.dirset(0).write(|w| w.dirsetp().bits(1 << 26 | 1 << 31 | 1 << 14)); // Blue, red, green

        // DO NOT REMOVE
        cortex_m::asm::delay(1000000000);

        compiler_fence(core::sync::atomic::Ordering::SeqCst);

        let ahb_secure_ctrl = &*AhbSecureCtrl::ptr().add(0x1000_0000);

        // let shadow_dcfg_cc_socu = 0x4013017C as *mut u32;
        // shadow_dcfg_cc_socu
        //     .write_volatile(shadow_dcfg_cc_socu.read_volatile() | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11));

        // AHB_SECURE_CTRL->APB_GRP0_MEM_RULE0      = 0xFCFFFFFFU;
        // AHB_SECURE_CTRL->APB_GRP0_MEM_RULE1      = 0xCCFFFFFFU;
        // AHB_SECURE_CTRL->APB_GRP1_MEM_RULE0      = 0xFCCFFFFFU;
        // AHB_SECURE_CTRL->APB_GRP1_MEM_RULE1      = 0xCCCCCCCCU;
        // AHB_SECURE_CTRL->APB_GRP1_MEM_RULE2      = 0xFCFFFFFCU;
        // AHB_SECURE_CTRL->AHB_PERIPH0_SLAVE_RULE0 = 0xFCCCFCCCU;
        // AHB_SECURE_CTRL->AIPS_BRIDGE0_MEM_RULE0  = 0xFFFCCCCCU;
        // AHB_SECURE_CTRL->AHB_PERIPH1_SLAVE_RULE0 = 0xCCCCCCCCU;
        // AHB_SECURE_CTRL->AIPS_BRIDGE1_MEM_RULE0  = 0xCCFCFFFFU;
        // AHB_SECURE_CTRL->AIPS_BRIDGE1_MEM_RULE1  = 0xFFFFCCCCU;
        // AHB_SECURE_CTRL->AHB_PERIPH2_SLAVE_RULE0 = 0xFFFFCCCFU;
        // AHB_SECURE_CTRL->AHB_PERIPH3_SLAVE_RULE0 = 0xFFFCCFCCU;
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

        // AHB_SECURE_CTRL->MASTER_SEC_LEVEL          = 0x80000000U;
        // AHB_SECURE_CTRL->MASTER_SEC_LEVEL_ANTI_POL = 0xBFFFFFFFU;
        ahb_secure_ctrl.master_sec_level().write(|w| w.bits(0x80000000));
        ahb_secure_ctrl
            .master_sec_level_anti_pol()
            .write(|w| w.bits(0xBFFFFFFF));

        // TODO gpio mask

        // NVIC->ITNS[0] = 0;
        // NVIC->ITNS[1] = 0;
        cp.NVIC.itns[0].write(0);
        cp.NVIC.itns[1].write(0);

        // SCB->AIRCR = (SCB->AIRCR & 0x000009FF7U) | 0x005FA0000U;
        // SCB->SCR &= 0x0FFFFFFF7U;
        // SCB->SHCSR &= 0x0FFF7FFFFU;
        cp.SCB.aircr.write((cp.SCB.aircr.read() & 0x000009FF7) | 0x005FA0000);
        cp.SCB.scr.modify(|w| w & 0x0FFFFFFF7);
        cp.SCB.shcsr.modify(|w| w & 0x0FFF7FFFF);

        // SCB->NSACR                        = 0x00000C03U;
        let nsacr = 0xE000ED8C as *mut u32;
        nsacr.write_volatile(0x00000C03);

        // SCnSCB->CPPWR                     = 0;
        let scn_scb = &*ScnScb::ptr().add(0x1000_0000);
        scn_scb.cppwr().write(|w| w.bits(0));

        // AHB_SECURE_CTRL->SEC_MASK_LOCK    = (AHB_SECURE_CTRL->SEC_MASK_LOCK & 0x0FFFCFFC0U) | 0x00002002AU;
        ahb_secure_ctrl
            .sec_mask_lock()
            .write(|w| w.bits((ahb_secure_ctrl.sec_mask_lock().read().bits() & 0x0FFFCFFC0) | 0x00002002A));

        // AHB_SECURE_CTRL->MASTER_SEC_LEVEL = (AHB_SECURE_CTRL->MASTER_SEC_LEVEL & 0x03FFFFFFFU) | 0x080000000U;
        // AHB_SECURE_CTRL->MASTER_SEC_LEVEL_ANTI_POL =
        //     (AHB_SECURE_CTRL->MASTER_SEC_LEVEL_ANTI_POL & 0x03FFFFFFFU) | 0x080000000U;
        ahb_secure_ctrl
            .master_sec_level()
            .write(|w| w.bits((ahb_secure_ctrl.master_sec_level().read().bits() & 0x03FFFFFFF) | 0x080000000));
        ahb_secure_ctrl
            .master_sec_level_anti_pol()
            .write(|w| w.bits((ahb_secure_ctrl.master_sec_level_anti_pol().read().bits() & 0x03FFFFFFF) | 0x080000000));

        // AHB_SECURE_CTRL->CM33_LOCK_REG    = 0x800002AAU;
        ahb_secure_ctrl.cm33_lock_reg().write(|w| w.bits(0x800002AA));

        // AHB_SECURE_CTRL->MISC_CTRL_REG    = 0x0000AAA5U;
        // AHB_SECURE_CTRL->MISC_CTRL_DP_REG = 0x0000AAA5U;
        ahb_secure_ctrl.misc_ctrl_reg().write(|w| w.bits(0x0000AAA5));
        ahb_secure_ctrl.misc_ctrl_dp_reg().write(|w| w.bits(0x0000AAA5));

        // Create the right function pointer to the reset vector
        // let nonsecure_reset =
        //     core::mem::transmute::<*const u32, extern "cmse-nonsecure-call" fn()>(nonsecure_reset as *const u32);

        gpio.set(0).write(|w| w.setp().bits(1 << 26));

        loop {
            cortex_m::asm::nop();
        }
    }
}

#[cortex_m_rt::exception]
unsafe fn SecureFault() -> ! {
    let sau = &*cortex_m::peripheral::SAU::PTR;
    // rprintln!(
    //     "SecureFault! - SFSR: {:#010X}, SFAR: {:#010X}",
    //     sau.sfsr.read().0,
    //     sau.sfar.read().0
    // );
    loop {
        cortex_m::asm::nop();
    }
}

#[cortex_m_rt::exception(trampoline = false)]
unsafe fn HardFault() -> ! {
    let gpio = &*Gpio::ptr();
    gpio.set(0).write(|w| w.setp().bits(1 << 31));
    loop {
        cortex_m::asm::nop();
    }
}

#[interrupt]
unsafe fn SECUREVIOLATION() {
    // rprintln!("SecureViolation!");
    loop {
        cortex_m::asm::nop();
    }
}

#[panic_handler]
fn panic(i: &PanicInfo) -> ! {
    // rprintln!("{}", i);
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
