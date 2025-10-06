#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use embassy_executor::Spawner;
use embassy_imxrt::gpio;
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

    rtt_target::set_print_channel(unsafe { UpChannel::conjure(0) }.unwrap());
    rtt_target::rprintln!("Hello world");

    let mut led = gpio::Output::new(
        p.PIO0_26,
        gpio::Level::Low,
        gpio::DriveMode::PushPull,
        gpio::DriveStrength::Normal,
        gpio::SlewRate::Standard,
    );

    loop {
        rprintln!("Toggling LED");
        led.toggle();
        Timer::after_millis(1000).await;
        rprintln!("Secure stuff: {}", do_stuff_secure(5));
    }
}
