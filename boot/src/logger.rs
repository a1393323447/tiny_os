use core::ptr;
use core::fmt::{self, Write};

use spinning_top::Spinlock;
use conquer_once::spin::OnceCell;

/// The global logger instance used for the `log` crate.
pub static LOGGER: OnceCell<LockedLogger> = OnceCell::uninit();

/// A [`Logger`] instance protected by a spinlock.
pub struct LockedLogger(Spinlock<Logger>);

pub const VGA_BUFFER_START_ADDR: usize = 0x0b8000;

pub const VGA_BUFFER_SIZE: usize = 80 * 25 * 2;


impl LockedLogger {
    pub fn new() -> Self {
        let mut logger = Logger::new();
        logger.clear();
        LockedLogger(Spinlock::new(logger))
    }
    /// Force-unlocks the logger to prevent a deadlock.
    ///
    /// This method is not memory safe and should be only used when absolutely necessary.
    pub unsafe fn force_unlock(&self) {
        self.0.force_unlock();
    }
}

impl log::Log for LockedLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut logger = self.0.lock();
        writeln!(logger, "{}:    {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}

pub struct Logger {
    vga_buffer: &'static mut [u8],
    x: usize,
    y: usize,
}

pub const VGA_TEXT_MODE_HEIGHT: usize = 25;
pub const VGA_TEXT_MODE_WIDTH:  usize = 80;
const GREEN_CHAR_ATTR: u8 = 0x0a;

impl Logger {
    fn new() -> Logger {
        let vga_buffer =  unsafe { 
            &mut *core::ptr::slice_from_raw_parts_mut(VGA_BUFFER_START_ADDR as *mut u8, VGA_BUFFER_SIZE) 
        };
        Logger { vga_buffer, x: 0, y: 0 }
    }

    fn carriage_return(&mut self) {
        self.x = 0;
    }

    fn newline(&mut self) {
        self.carriage_return();
        
        self.y += 1;
        
        if self.y >= VGA_TEXT_MODE_HEIGHT {
            self.clear();
        }
    }

    fn write_char(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            ch => {
                if self.x >= VGA_TEXT_MODE_WIDTH * 2 {
                    self.newline();
                }
                let pos = self.y * VGA_TEXT_MODE_WIDTH * 2 + self.x;
                self.vga_buffer[pos] = ch as u8;
                self.vga_buffer[pos + 1] = GREEN_CHAR_ATTR;
                self.x += 2;
                // 防止编译器优化
                let _ = unsafe { ptr::read_volatile(&self.vga_buffer[pos] as *const u8) };
                let _ = unsafe { ptr::read_volatile(&self.vga_buffer[pos + 1] as *const u8) };
            }
        }
    }

    /// 清屏
    pub fn clear(&mut self) {
        self.x = 0;
        self.y = 0;
        self.vga_buffer.fill(0);
    }


}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}