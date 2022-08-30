#![no_std]
#![no_main]

use core::{ops, slice};

/// This structure represents the information that the bootloader passes to the kernel.
///
/// The information is passed as an argument to the entry point. The entry point function must
/// have the following signature:
///
/// ```
/// # use bootloader::BootInfo;
/// # type _SIGNATURE =
/// extern "C" fn(boot_info: &'static mut BootInfo) -> !;
/// ```
///
/// Note that no type checking occurs for the entry point function, so be careful to
/// use the correct argument types. To ensure that the entry point function has the correct
/// signature, use the [`entry_point`] macro.
#[derive(Debug)]
#[repr(C)]
pub struct BootInfo {
    /// A map of the physical memory regions of the underlying machine.
    ///
    /// The bootloader queries this information from the BIOS/UEFI firmware and translates this
    /// information to Rust types. It also marks any memory regions that the bootloader uses in
    /// the memory map before passing it to the kernel. Regions marked as usable can be freely
    /// used by the kernel.
    pub memory_regions: MemoryRegions,
    /// Information about the framebuffer for screen output.
    pub framebuffer: FrameBuffer,
    /// The address of the `RSDP` data structure, which can be use to find the ACPI tables.
    ///
    /// This field is `None` if no `RSDP` was found (for BIOS) or reported (for UEFI).
    pub rsdp_addr: Optional<u64>,
    /// The thread local storage (TLS) template of the kernel executable, if present.
    pub tls_template: Optional<TlsTemplate>,
}

/// Represents the different types of memory.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryRegionKind {
    /// Unused conventional memory, can be used by the kernel.
    Usable,
    /// Memory mappings created by the bootloader, including the kernel and boot info mappings.
    ///
    /// This memory should _not_ be used by the kernel.
    Bootloader,
    /// An unknown memory region reported by the BIOS firmware.
    ///
    /// This should only be used if the BIOS memory type is known as usable.
    Unknown(u32),
}

/// Represent a physical memory region.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    /// The physical start address of the region.
    pub start: u64,
    /// The physical end address (exclusive) of the region.
    pub end: u64,
    /// The memory type of the memory region.
    ///
    /// Only [`Usable`][MemoryRegionKind::Usable] regions can be freely used.
    pub kind: MemoryRegionKind,
}

impl MemoryRegion {
    /// Creates a new empty memory region (with length 0).
    pub const fn empty() -> Self {
        MemoryRegion {
            start: 0,
            end: 0,
            kind: MemoryRegionKind::Bootloader,
        }
    }
}

/// FFI-safe slice of [`MemoryRegion`] structs, semantically equivalent to
/// `&'static mut [MemoryRegion]`.
///
/// This type implements the [`Deref`][core::ops::Deref] and [`DerefMut`][core::ops::DerefMut]
/// traits, so it can be used like a `&mut [MemoryRegion]` slice. It also implements [`From`]
/// and [`Into`] for easy conversions from and to `&'static mut [MemoryRegion]`.
#[derive(Debug)]
#[repr(C)]
pub struct MemoryRegions {
    pub(crate) ptr: *mut MemoryRegion,
    pub(crate) len: usize,
}

impl ops::Deref for MemoryRegions {
    type Target = [MemoryRegion];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl ops::DerefMut for MemoryRegions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl From<&'static mut [MemoryRegion]> for MemoryRegions {
    fn from(regions: &'static mut [MemoryRegion]) -> Self {
        MemoryRegions {
            ptr: regions.as_mut_ptr(),
            len: regions.len(),
        }
    }
}

impl From<MemoryRegions> for &'static mut [MemoryRegion] {
    fn from(regions: MemoryRegions) -> &'static mut [MemoryRegion] {
        unsafe { slice::from_raw_parts_mut(regions.ptr, regions.len) }
    }
}

/// A pixel-based framebuffer that controls the screen output.
#[derive(Debug)]
#[repr(C)]
pub struct FrameBuffer {
    pub buffer_start: u64,
    pub buffer_byte_len: usize,
    pub info: FrameBufferInfo,
}

impl FrameBuffer {
    /// Returns the raw bytes of the framebuffer as slice.
    pub fn buffer(&self) -> &[u8] {
        unsafe { self.create_buffer() }
    }

    /// Returns the raw bytes of the framebuffer as mutable slice.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe { self.create_buffer() }
    }

    unsafe fn create_buffer<'a>(&self) -> &'a mut [u8] {
        slice::from_raw_parts_mut(self.buffer_start as *mut u8, self.buffer_byte_len)
    }

    /// Returns layout and pixel format information of the framebuffer.
    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }
}

/// Describes the layout and pixel format of a framebuffer.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    /// The total size in bytes.
    pub byte_len: usize,
    /// The width in pixels.
    pub horizontal_resolution: usize,
    /// The height in pixels.
    pub vertical_resolution: usize,

}

/// Information about the thread local storage (TLS) template.
///
/// This template can be used to set up thread local storage for threads. For
/// each thread, a new memory location of size `mem_size` must be initialized.
/// Then the first `file_size` bytes of this template needs to be copied to the
/// location. The additional `mem_size - file_size` bytes must be initialized with
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TlsTemplate {
    /// The virtual start address of the thread local storage template.
    pub start_addr: u64,
    /// The number of data bytes in the template.
    ///
    /// Corresponds to the length of the `.tdata` section.
    pub file_size: u64,
    /// The total number of bytes that the TLS segment should have in memory.
    ///
    /// Corresponds to the combined length of the `.tdata` and `.tbss` sections.
    pub mem_size: u64,
}

/// FFI-safe variant of [`Option`].
///
/// Implements the [`From`] and [`Into`] traits for easy conversion to and from [`Option`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum Optional<T> {
    /// Some value `T`
    Some(T),
    /// No value
    None,
}

impl<T> Optional<T> {
    /// Converts the `Optional` to an [`Option`].
    pub fn into_option(self) -> Option<T> {
        self.into()
    }

    /// Converts from `&Optional<T>` to `Option<&T>`.
    ///
    /// For convenience, this method directly performs the conversion to the standard
    /// [`Option`] type.
    pub const fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Some(x) => Some(x),
            Self::None => None,
        }
    }

    /// Converts from `&mut Optional<T>` to `Option<&mut T>`.
    ///
    /// For convenience, this method directly performs the conversion to the standard
    /// [`Option`] type.
    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Some(x) => Some(x),
            Self::None => None,
        }
    }
}

impl<T> From<Option<T>> for Optional<T> {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => Optional::Some(v),
            None => Optional::None,
        }
    }
}

impl<T> From<Optional<T>> for Option<T> {
    fn from(optional: Optional<T>) -> Option<T> {
        match optional {
            Optional::Some(v) => Some(v),
            Optional::None => None,
        }
    }
}

/// Check that bootinfo is FFI-safe
extern "C" fn _assert_ffi(_boot_info: BootInfo) {}
