#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

pub mod logger;
pub mod interrupts;
pub mod gdt;