#![no_std]
#![no_main]

use kernel::logger;
use kernel::interrupts::init_idt;

use core::{
    arch::asm,
    panic::PanicInfo,
};
use boot_info::BootInfo;



#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    logger::init_logger(&boot_info.framebuffer);

    init_idt();

    // 触发断点
    x86_64::instructions::interrupts::int3();

    // 触发 double fault
    unsafe {
        *(0x114514 as *mut u64) = 24;
    };

    panic!("DEAK LOCK");
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
        unsafe { asm!("cli; hlt") };
    }
}
