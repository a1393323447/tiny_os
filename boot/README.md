# boot

## 简介
`boot` 目录下是 `tiny OS` 的 `bootloader` 的源码。该过程主要分为四个阶段。

## stage 1
第一阶段对应文件 `src/asm/stage_1.s` 。该阶段主要完成：
- [x] 开启 A20 地址线
- [x] 获取 4 GB 内存寻址能力, 为读取 bootloader 剩余部分, 加载内核做准备
      方法: 设置 32 位 GDT, 进入保护模式, 并设置好数据段的段选择子, 再返回实模式
- [x] 将剩余的 bootloader 从硬盘中读出
- [x] 跳转到 `stage 2` 执行

## stage 2
第二阶段从实模式跨进保护模式，对应文件 `src/asm/stage_2_real.s` 和 `src/asm/stage_2_protected.s` 。该阶段主要完成：
- [x] 通过 int 0x15 e820 中断, 获取内存布局, 并保存 
- [x] 从硬盘加载内核到内存
      方法: 通过硬盘端口读取硬盘, 通过位于 0x0500 处的 _kernel_buf 将内核 ELF 文件加载到内存 0x400000 处
- [x] 再次进入保护模式, 设置代码段选择子, 跳转到 `stage 3`


## stage 3
第三阶段对应文件 `src/asm/stage_3.s` 。该阶段主要完成：
- [x] 检查 cpu 是否支持 CPUID 指令
- [x] 检查 cpu 是否支持 long mode
- [x] 建立四级页表, 开启内存分页管理
- [x] 进入 long mode , 跳转到 stage_4(定义在 `src/main.rs` )

## stage 4
第四阶段的入口 `stage_4` 定义在文件 `src/main.rs`，由所有的 `Rust` 源文件组成。该阶段主要完成：
- [x] 通过在 `stage 2` 中获得的 `memory_map` ，为所有可用物理内存作映射。
- [x] 为内核新建一个页表
- [x] 解析位于内存 `0x400000` 处的内核的 `elf` 文件，并在内核的页表中，将所有的 `section` 都映射到相应的虚拟地址上
- [x] 将 `VGA Text Buffer` 映射可用的虚拟地址空间上
- [x] 准备 `BootInfo`
- [x] 将 `cr3` 设置为内核的页表，并跳转到内核执行

## 杂项说明
在 `src/asm` 目录下还有没有用到的汇编文件，它们是我在写 `bootLoader` 时一些尝试，虽然最终没有用上，但还是很有保存的价值：
- `int0x13.s` : 通过 `BIOS` 中断读取硬盘文件
- `load.s` : 通过 `BIOS` 中断加载内核
- `vesa.s` : 引用自 https://gitlab.redox-os.org/redox-os/bootloader ，是用于获取显卡信息，配置 `VESA` 显式模式的，但最终决定先在字符模式下，初步完成 `tiny OS` 再考虑图形模式

## 参考
`tiny OS` 的 `bootloader` 主要参考了 [rust-osdev/bootloader](https://github.com/rust-osdev/bootloader)。（在写 `bootloader` 的过程中，看了一两千行汇编，真的可以说是 `debug` 到头秃，没有参考，可能我永远完不成这部分，非常感谢这些开源大佬）。