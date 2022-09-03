use core::ptr;
use core::fmt::{self, Write};

use boot_info::FrameBuffer;
use spinning_top::Spinlock;
use conquer_once::spin::OnceCell;


const VGA_TEXT_MODE_HEIGHT: usize = 25;
const VGA_TEXT_MODE_WIDTH:  usize = 80;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Color {
    Black      = 0x0,
    Blue       = 0x1,
    Green      = 0x2,
    Cyan       = 0x3,
    Red        = 0x4,
    Magenta    = 0x5,
    Brown      = 0x6,
    LightGray  = 0x7,
/*  Only work for foreground color  */
    DarkGray   = 0x8,
    LightBlue  = 0x9,
    LightGreen = 0xa,
    LightCyan  = 0xb,
    LightRed   = 0xc,
    Pink       = 0xd,
    Yellow     = 0xe,
    White      = 0xf,
}

#[derive(Clone, Copy)]
struct Attribute {
    blink: bool,
    fg: Color,
    bg: Color,
}

impl Default for Attribute {
    fn default() -> Attribute {
        Attribute { 
            blink: false, 
            fg: Color::White, 
            bg: Color::DarkGray 
        }
    }
}

impl Attribute {
    fn set_blink(&mut self, blink: bool) -> &mut Attribute {
        self.blink = blink;
        
        self
    }

    fn set_fg(&mut self, fg: Color) -> &mut Attribute {
        self.fg = fg;

        self
    }

    fn set_bg(&mut self, bg: Color) -> &mut Attribute {
        self.bg = bg;

        self
    }

    const fn to_code(&self) -> u8 {
        let mut code = (self.blink as u8) << 7;
        code |=  self.fg as u8;
        code |= (self.bg as u8) & 0x7f;

        code
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct ColorCode(u8);

impl From<Attribute> for ColorCode {
    fn from(attr: Attribute) -> Self {
        ColorCode(attr.to_code())
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Cell {
    ascii_char: u8,
    color_code: ColorCode,
}

impl Cell {
    fn new(ch: char, code: ColorCode) -> Cell {
        Cell { ascii_char: ch as u8, color_code: code }
    }
}


/// The global logger instance used for the `log` crate.
pub static LOGGER: OnceCell<LockedLogger> = OnceCell::uninit();

/// A [`Logger`] instance protected by a spinlock.
pub struct LockedLogger(Spinlock<Logger>);

impl LockedLogger {
    pub fn new(framebuffer: &FrameBuffer) -> Self {
        let mut logger = Logger::new(framebuffer);

        logger.clear();

        LockedLogger(Spinlock::new(logger))
    }

    pub fn clear(&mut self) {
        self.0.lock().clear();
    }

    pub fn set_bg(&mut self, bg: Color) -> &mut Self {
        let mut locked_logger = self.0.lock();
        locked_logger.attr.set_bg(bg);

        drop(locked_logger);

        self
    }

    pub fn set_fg(&mut self, fg: Color) -> &mut Self {
        let mut locked_logger = self.0.lock();
        locked_logger.attr.set_fg(fg);

        drop(locked_logger);

        self
    }

    pub fn set_blink(&mut self, blink: bool) -> &mut Self {
        let mut locked_logger = self.0.lock();
        locked_logger.attr.set_blink(blink);

        drop(locked_logger);

        self
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
        use x86_64::instructions::interrupts;

        interrupts::without_interrupts(|| {
            let mut logger = self.0.lock();

            let fg = logger.attr.fg;

            match record.level() {
                log::Level::Error => logger.set_fg(Color::Red),
                log::Level::Warn  => logger.set_fg(Color::Yellow),
                log::Level::Info  => logger.set_fg(Color::LightBlue),
                log::Level::Debug => logger.set_fg(Color::LightGreen),
                log::Level::Trace => logger.set_fg(Color::White),
            };

            writeln!(logger, "{}: {}", record.level(), record.args()).unwrap();

            logger.set_fg(fg);
        });
    }

    fn flush(&self) {}
}

struct Logger {
    framebuffer: &'static mut [Cell],
    x: usize,
    y: usize,
    attr: Attribute,
}

impl Logger {
    fn new(framebuffer: &FrameBuffer) -> Logger {
        let framebuffer_addr = framebuffer.buffer_start as *mut Cell;

        let framebuffer = unsafe {
            &mut *ptr::slice_from_raw_parts_mut(framebuffer_addr, VGA_TEXT_MODE_WIDTH * VGA_TEXT_MODE_HEIGHT)
        };

        Logger { 
            framebuffer, 
            x: 0, y: 0, 
            attr: Default::default() 
        }
    }

    #[allow(unused)]
    fn set_blink(&mut self, blink: bool) -> &mut Attribute {
        self.attr.set_blink(blink);

        &mut self.attr
    }

    #[allow(unused)]
    fn set_bg(&mut self, bg: Color) -> &mut Attribute {
        self.attr.set_bg(bg);
        
        &mut self.attr
    }

    fn set_fg(&mut self, fg: Color) -> &mut Attribute {
        self.attr.set_fg(fg);
        
        &mut self.attr
    }

    fn carriage_return(&mut self) {
        self.x = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = Attribute {
            blink: false,
            fg: self.attr.bg,
            bg: self.attr.bg,
        };

        let blank_cell = Cell::new(' ', blank.into());

        for col in 0..VGA_TEXT_MODE_WIDTH {
            self.framebuffer[row * VGA_TEXT_MODE_WIDTH + col] = blank_cell;
        }
    }

    fn newline(&mut self) {
        self.carriage_return();
        
        self.y += 1;
        
        if self.y >= VGA_TEXT_MODE_HEIGHT {
            self.roll();
        }
    }

    fn roll(&mut self) {
        for row in 1..VGA_TEXT_MODE_HEIGHT {
            for col in 0..VGA_TEXT_MODE_WIDTH {
                let cell = self.framebuffer[row * VGA_TEXT_MODE_WIDTH + col];
                self.framebuffer[(row - 1) * VGA_TEXT_MODE_WIDTH + col] = cell;
            }
        }

        self.y = VGA_TEXT_MODE_HEIGHT - 1;
        self.clear_row(VGA_TEXT_MODE_HEIGHT - 1);
        self.x = 0;
    }

    /// 清屏
    fn clear(&mut self) {
        self.x = 0;
        self.y = 0;

        let blank = Attribute {
            blink: false,
            fg: self.attr.bg,
            bg: self.attr.bg,
        };

        let blank_cell = Cell::new(' ', blank.into());

        
        self.framebuffer.fill(blank_cell);
    }

    // TODO: 修改成仅支持 ascii
    fn write_char(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            ch => {
                if self.x >= VGA_TEXT_MODE_WIDTH {
                    self.newline();
                }
                let pos = self.y * VGA_TEXT_MODE_WIDTH + self.x;
                self.framebuffer[pos] = Cell::new(ch, self.attr.into());
                self.x += 1;
                // 防止编译器优化
                let _ = unsafe { ptr::read_volatile(&self.framebuffer[pos] as *const Cell) };
            }
        }
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

pub fn init_logger(framebuffer: &FrameBuffer) {
    let logger = LOGGER.get_or_init(move || LockedLogger::new(framebuffer));
    
    log::set_logger(logger).expect("logger already set");
    log::set_max_level(log::LevelFilter::max());
}