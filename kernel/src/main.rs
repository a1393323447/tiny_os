#![no_std]
#![no_main]

use kernel::logger;

use core::panic::PanicInfo;
use boot_info::BootInfo;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    kernel::init(&boot_info);
    
    log::info!("Running in kernel");
    log::info!("{:#?}", boot_info);

    kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        logger::LOGGER
            .get()
            .map(|l| l.force_unlock())
    };
    log::error!("{}", info);

    loop {
        kernel::hlt_loop();
    }
}
