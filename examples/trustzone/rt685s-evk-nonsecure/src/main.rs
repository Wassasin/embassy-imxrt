#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use embassy_executor::Spawner;
use embassy_imxrt::dma::transfer::TransferOptions;
use embassy_imxrt::gpio;
use embassy_imxrt::pac::{interrupt, Interrupt};
use embassy_time::Timer;
use panic_probe as _;
use rtt_target::{rprintln, UpChannel};

#[repr(C)]
pub struct RttControlBlock {
    header: rtt_target::rtt::RttHeader,
    up_channels: [rtt_target::rtt::RttChannel; 1],
    down_channels: [rtt_target::rtt::RttChannel; 0],
}

#[used]
#[export_name = "_SEGGER_RTT"]
#[link_section = ".shared_rtt.header"]
pub static mut CONTROL_BLOCK: MaybeUninit<RttControlBlock> = MaybeUninit::uninit();

unsafe extern "C" {
    safe fn do_stuff_secure(num: u32) -> u32;
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());
    let mut cp = cortex_m::Peripherals::take().unwrap();

    // Make DMA0 lower priority than SECUREVIOLATION.
    unsafe {
        cp.NVIC.set_priority(Interrupt::DMA0, 32);
    }

    rtt_target::set_print_channel(unsafe { UpChannel::conjure(0) }.unwrap());
    rtt_target::rprintln!("Hello world");

    let mut led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    let channel = embassy_imxrt::dma::Dma::reserve_channel(p.DMA0_CH0).unwrap();

    rprintln!("Toggling LED");
    led.toggle();
    Timer::after_millis(1000).await;
    rprintln!("Toggling LED");
    led.toggle();

    rprintln!("About to call secure function...");
    rprintln!("Calling secure function: {}", do_stuff_secure(5));

    rprintln!("Trying to get DMA access to secure memory");
    let mut counter = 0u32;
    embassy_imxrt::dma::transfer::Transfer::new_raw_transfer(
        &channel,
        embassy_imxrt::dma::transfer::Direction::MemoryToMemory,
        0x20000000 as *const u32,
        &raw mut counter,
        4,
        TransferOptions {
            width: embassy_imxrt::dma::transfer::Width::Bit32,
            priority: embassy_imxrt::dma::transfer::Priority::Priority0,
        },
    )
    .await;

    rprintln!("counter: {:X?}", counter);

    counter += 1;

    embassy_imxrt::dma::transfer::Transfer::new_raw_transfer(
        &channel,
        embassy_imxrt::dma::transfer::Direction::MemoryToMemory,
        &raw const counter,
        0x20000000 as *mut u32,
        4,
        TransferOptions {
            width: embassy_imxrt::dma::transfer::Width::Bit32,
            priority: embassy_imxrt::dma::transfer::Priority::Priority0,
        },
    )
    .await;

    rprintln!("Oh no! The DMA worked!: {}", do_stuff_secure(5));

    rprintln!("Trying to get CPU access to secure memory");
    counter = unsafe { (0x20000000 as *const u32).read_volatile() };
    rprintln!("counter: {:X?}", counter);
    counter += 1;
    unsafe { (0x20000000 as *mut u32).write_volatile(counter) }
    rprintln!("Oh no! The CPU worked!: {}", do_stuff_secure(5));

    cortex_m::asm::bkpt();
}

#[interrupt]
unsafe fn SECUREVIOLATION() {
    rprintln!("SECUREVIOLATION!");
    loop {
        cortex_m::asm::nop();
    }
}
