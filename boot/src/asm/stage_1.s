# 定义一个可读写、可执行段
# 定义在 linker.ld, 处于 bootloader 的最开头
.section .boot-first-stage, "awx"
.global  _start
.code16

# stage 1
# 1. 开启 A20 地址线                                                           [√]
# 3. 获取 4 GB 内存寻址能力, 为读取 bootloader 剩余部分, 加载内核做准备          [√]
#    方法: 设置 32 位 GDT, 进入保护模式, 并设置好数据段选择子, 再返回实模式
# 4. 将剩余的 bootloader 从硬盘中读出                                           [√]
# 5. 跳转执行                                                                  [√]

.equ VGA_START_ADDR, 0xb800  # VGA 字符缓冲区起始地址
.equ CHAR_ATTR,      0x0a    # 绿色字符
.equ HDDPORT,        0x1f0   # 硬盘端口

# 三个段选择子 -> DPL 都为 00
.equ CODE_SELECTOR,   1 << 3
.equ DATA_SELECTOR,   2 << 3
.equ VIDEO_SELECTOR,  3 << 3 # only use in 32 bit
.equ KERNEL_CODE_START_ADDR, 0x400000

_start:
    # 将各个段寄存器清 0
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # cld: clear direction flag
    # 在使用串操作指令 (lodsb, stosb) 时, 作为索引的 si、di 会自动变化
    # 方向标志位用于指示变化的方向
    # 当方向标志位(CF) = 0 时, 向高地址增长
    # 当方向标志位(CF) = 1 时, 向低地址减少
    # cld 用于将 CF 置为 0
    cld

    # 初始化栈
    mov sp, 0x7c00

    mov cx, 12
    mov si, offset boot_start_str
    call print_real_mode

enable_a20:
    # 开启 a20 地址线
    # 将端口 0x92 的第 2 个二进制位置为 1
    in al, 0x92         # 南桥芯片上的端口
    test al, 2          # 有些 BIOS 默认开启 a20 地址线
    jnz enable_a20_after    # 如果已经开启则跳过
    or al, 2
    and al, 0xfe
    out 0x92, al
enable_a20_after:

# 获取 4GB 寻址空间
# ref: https://www.zhihu.com/question/21130837
# 为什么需要 4GB 寻址空间 ?
# 在实模式下, 开启 A20 地址线后, cpu 的寻址空间 为 1MB (0X10000) ,
# 而 由于实模式内存布局的限制, bootloader 只能从地址 0x7c00 开始加载, 
# 而在 debug 模式下编译的 bootloader 由于没有优化以及保留了 debug 信息,
# 大小会达到 130 KiB, 从 0x7c00 处开始加载会导致超过 cpu 寻址能力,
# 所以需要获取 4GB 寻址空间
store_real_mode_regs:
    push ds
    push es

enter_protected_mode:
    cli
    lgdt [gdt32info] # 加载全局描述符表

    # 将 cr0 的 PE 位设置为 1 , 进入保护模式
    xor eax, eax
    mov eax, cr0
    or  eax, 1
    mov cr0, eax

set_selectors:
    mov bx, DATA_SELECTOR
    mov ds, bx
    mov es, bx

back_to_real_mode:
    and al, 0xfe    # clear protected mode bit
    mov cr0, eax

real_mode:
    pop es # get back old extra segment
    pop ds # get back old data segment

    sti
    # back to real mode, but internal data segment register is still loaded
    # with gdt segment -> we can access the full 4GiB of memory

# https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
# 有些机器不支持扩展 bios 中断, 所以要先检查机器是否支持
check_int13h_extensions:
    mov ah, 0x41
    mov bx, 0x55aa
    # dl contains drive number
    int 0x13
    jc port_read_hdd

# To read a disk:
#   Set the proper values in the disk address packet structure
#   Set DS:SI -> Disk Address Packet in memory
#   Set AH = 0x42
#   Set DL = "drive number" -- typically 0x80 for the "C" drive
#   Issue an INT 0x13.
# The carry flag will be set if there is any error during the transfer. AH should be set to 0 on success.
# To write to a disk, set AH = 0x43.
# 理论上我们最多一次读 127 个 sector -> 127 * 512 = 65024 字节
load_rest_of_bootloader_from_disk:
    mov eax, offset _rest_of_bootloader_start_addr

    mov ecx, 0   # 当前加载的部分相对于 _rest_of_bootloader_start_addr 的偏移
                 # 以字节为单位
load_from_disk:
    mov eax, offset _rest_of_bootloader_start_addr
    add eax, ecx # 获取当前加载到的地址

    # dap buffer segment
    mov ebx, eax
    shr ebx, 4 # 物理地址 = 段基地址 * 16 + 偏移地址, 故段基地址要除以 16
    mov [dap_buf_seg], bx

    # buffer offset
    # 偏移地址 = 物理地址 - 段基地址 * 16
    shl ebx, 4 # multiply by 16
    sub eax, ebx
    mov [dap_buf_offset], ax

    mov eax, offset _rest_of_bootloader_start_addr
    add eax, ecx # eax = 当前加载到的地址

    # number of disk blocks to load
    mov ebx, offset _rest_of_bootloader_end_addr
    sub ebx, eax # ebx = end - start = 剩下的需要加载的字节数
    jz load_from_disk_done # 如果等于 0 则加载完成
    shr ebx, 9 # div 512
               # linker.ld 中 _rest_of_bootloader_start_addr 紧接着 .boot-first-stage ,
               # _rest_of_bootloader_end_addr 以 512 字节对齐
               # 而 .boot-first-stage 正好 512 字节
               # 故它们的差也一定是 512 字节的倍数, 一定整除, 不用向上取整
    cmp ebx, 0xff # ebx <= 0xff(一次最多读 0xff 个扇区) ?
    jle .continue_loading_from_disk # 如果是, 则读 ebx 个扇区
    mov ebx, 0xff # 如果 ebx > 0xff, 则一次只能读 0xff 个扇区
.continue_loading_from_disk:
    mov [dap_sector_num], bx # 设置扇区数
    
    shl ebx, 9   # ebx * 512 = 读取的字节数
    add ecx, ebx # 更新 offset

    # 计算 LBA 号
    mov ebx, offset _start
    sub eax, ebx
    shr eax, 9 # divide by 512 (block size)
    mov [dap_start_lba], eax

    # BIOS 中断读取硬盘
    mov si, offset dap
    mov ah, 0x42
    int 0x13
    jc rest_of_bootloader_load_failed

    jmp load_from_disk # 循环

load_from_disk_done:
    # reset segment to 0
    mov word ptr [dap_buf_seg], 0

to_stage_2:
    mov eax, offset stage2
    jmp eax

port_read_hdd:
    mov ebx, offset _rest_of_bootloader_start_addr
    mov ecx, offset _rest_of_bootloader_end_addr
    sub ecx, ebx
    shr ecx, 9  # div 512
                # linker.ld 中 _rest_of_bootloader_start_addr 紧接着 .boot-first-stage ,
                # _rest_of_bootloader_end_addr 以 512 字节对齐
                # 而 .boot-first-stage 正好 512 字节
                # 故它们的差也一定是 512 字节的倍数, 一定整除, 不用向上取整
    mov ebx, 1  # LBA 1
    mov edi, offset _rest_of_bootloader_start_addr
    call port_read

    jmp to_stage_2

rest_of_bootloader_load_failed:
    jmp rest_of_bootloader_load_failed

# --------------------------------- func ----------------------------------------

# 功能: 用于在实模式下打印
# 输入: 
#       cx = 字符串长度
#       si = 字符串首地址
# 无返回值
print_real_mode:
    .rinit:
        push ax
        push es
        mov ax, VGA_START_ADDR
        mov es, ax
        xor di, di
    .rprint:
        lodsb al, byte ptr [si]
        stosb
        mov   al, CHAR_ATTR
        stosb
        loop .rprint
    .rend:
        pop es
        pop ax
        ret

# LBA 方式 
# Logical Block Addressing (逻辑块寻址)
# 使用 LBA28 寻址方式 (最大可寻址 128G)
# 1. 在 0x1f2 中写入要读取的扇区数
# 2. 通过 28 位 逻辑号告诉硬盘从哪个逻辑扇区开始读
#    因为端口(0x1f1 ~ 0x1f7)是 8 位的, 所以要用 4 个端口来表示
#
#    [0x1f6]   [0x1f5]   [0x1f4]   [0x1f3]
#     27~24     23~16     15~8       7~0
#    [0x1f6] -> [ 1|  | 1|  |27|26|25|24]
#                 7  6  5  4  3  2  1  0
#
#     第 4 位: 选择硬盘号   -> 0: 主硬盘  1: 从硬盘
#     第 6 位: 选择读写模式 -> 0: CHS     1: LBA
#
# 3. 在 0x1f7写入 0x20(表示读硬盘) / 0x30(表示写硬盘)
# 4. 等待硬盘就绪, 通过 0x1f7 端口查询硬盘状态
#    [0x1f7] -> [BSY|   |   |   |DRQ|   |   |   ]
#                 7   6   5   4   3   2   1   0
#     第 7 位 -> 0: 硬盘闲  1: 硬盘忙
#     第 3 位 -> 0: 未就绪  1: 已就绪
# 5. 一切就绪后, 在 0x1f0 端口(16位)读取数据(1 word)
# 参数: ecx: 读取扇区数
#       ebx: LBA 号 (逻辑号)
#       edi: 读取出的数据存放在内存的偏移地址
# --------------------------------------------------
#       si: 逻辑号低16位
#       bx: 逻辑号高12位
port_read:
    .setup:
        push eax # 保存现场
        push ebx
        push ecx
        push edx

    .check_conditions:         # ecx: 需要读取的扇区数
        xor eax, eax           # eax = 0
        cmp ecx, 0xff          # ecx >= 0xff ?  
        jge .set_selectors_num # Y -> .set_selectors_num
        cmp ecx, 0             # ecx >= 0 ?
        jle .return            # Y -> return
        mov eax, ecx
    .sendsignal:
        sub ecx, eax        # 需要读取的扇区数 - 即将读取的扇区数
        push ecx            # 保存剩余的需要读取的扇区数
        
        mov ecx, eax        # ecx = 即将读取的扇区数

        .set_LBA_num:
            push ebx        # 保存逻辑号
            mov si, bx      # 逻辑号低 16 位
            shr ebx, 16     # 逻辑号高 12 位

        mov dx, HDDPORT+2
        out dx, al          # 端口是 8 位寄存器, 所以用 al

        mov ax, si          # 将逻辑号的 0 ~ 15 位读入 ax 中

        mov dx, HDDPORT+3   # 写入 7 ~ 0 位
        out dx, al

        mov dx, HDDPORT+4   # 写入 15 ~ 8 位
        mov al, ah          # 将 15 ~ 8 位 从 ah 复制到 al 中
        out dx, al

        mov ax, bx          # 将逻辑号的 27 ~ 16 位读入 ax 中 
                            # 实际读入到 31 位

        mov dx, HDDPORT+5   # 写入 23 ~ 16 位
        out dx, al

        mov dx, HDDPORT+6   # 写入 27 ~ 24 位, 以及硬盘参数
        mov al, ah          # 将 25 ~ 31 位 从 ah 复制到 al 中 .... [....]
        mov ah, 0xe0        # 0xe0 -> [1110] 0000
        or  al, ah          # 将 al 和 ah 融合在一起 [1110] [....], 存在 al 中
        out dx, al          # 将 al 输出

        mov dx, HDDPORT+7   # 在 0x1f7 端口中写入 0x20 表示读盘
        mov al, 0x20
        out dx, al

        pop ebx             # 读取逻辑号
        add ebx, ecx        # 更新逻辑号

    .waits:
        in al, dx           # 读取 0x1f7 端口, 得到硬盘的状态字节
        and al, 0x88        # 0x88 -> 1000 1000 除了 3、7位的其它位置为 0
        cmp al, 0x08        # 0x08 -> 0000 1000 当第 7 位为 0 , 第 3 位为 1 时, 就可以读盘了 (cmp 会将两个操作数相减)
        jnz .waits          # 不等的话就继续等待, jump if zero flag is unset
    
    .readsetup:
        mov dx, HDDPORT     # 设置读取数据的端口 0x1f0

    .read_a_selector:
        push ecx            # 保存即将读取的扇区数
        mov ecx, 256        # 设置读取次数: 一个扇区 512 个字节, 读 256 次
        .readword:              # 读数据, 一次读 2 个字节 (0x1f0是16位端口)
            in ax, dx           # 使用 in 指令, 将数据读取到 ax 寄存器
            mov ds:[edi], ax    # 将数据保存到 ds:di 指向的内存单元
            add edi, 2          # 将偏移地址 +2 (一次写入 2 个字节)         
            loop .readword      # 循环 256 次, 读取一个扇区的数据
        pop ecx               # 读取即将读取的扇区数
        loop .read_a_selector # 循环并更新次数

    pop ecx                 # 读取 剩余的需要读取的扇区数
    jmp .check_conditions   # 检查循环条件

    .set_selectors_num:
        mov al, 0xff        # 剩余的需要读取的扇区数 >= 0xff -> 设置 al 为 0xff
        jmp .sendsignal

    .return:
        pop edx              # 还原现场
        pop ecx
        pop ebx
        pop eax

        ret

# ------------------------------------ data -------------------------------------
boot_start_str: .ascii "Booting(1)  "
    
# https://wiki.osdev.org/GDT
gdt32info: # (GDTR)
   .word gdt32_end - gdt32 - 1  # last byte in table
   .word gdt32                  # start of table

gdt32:
    # entry 0 is always unused
    .quad 0
codedesc32:              # 代码段描述符
    .word 0xffff       # 段界限: 0xffff_ff 
    .word 0x0000       # 段基址: 0x0000_00
    .byte 0            # (这两个字段都很分散)
    .byte 0x9a         # 段属性: a  -> Execute/Read, 段粒度: 4k -> G(1), 段权限: 0  -> DPL(00), P(1)
    .byte 0xcf         # G(1) -> 粒度为 4k (段界限只给出了数值, 还要乘上相应的粒度单位)
    .byte 0            # 
datadesc32:
    .word 0xffff       # 数据段描述符
    .word 0x0000       
    .byte 0            
    .byte 0x92         # 段属性: 2 -> Read/Write
    .byte 0xcf         # 其它同 代码段
    .byte 0
videodesc32:            # 视频段描述符 -> 映射到 VGA
    .word 0x7fff      # 段界限: 0x7ffff
    .word 0x8000      # 段基址: 0x000b8000
    .byte 0x0b        # 其它同 数据段
    .byte 0x92
    .byte 0xcf
    .byte 0
gdt32_end:

# Format of disk address packet       https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
# Offset	Size	Description
#  0	     1	 size of packet (16 bytes)
#  1	     1	 always 0
#  2	     2	 number of sectors to transfer (max 127 on some BIOSes) ---> 一次最多读 127 个 sector
#  4	     4	 transfer buffer (16 bit segment:16 bit offset) (see note #1)
#  8	     4	 lower 32-bits of 48-bit starting LBA
# 12	     4	 upper 16-bits of 48-bit starting LBA
# Notes:

# (1) The 16 bit segment value ends up at an offset of 6 from the beginning of the structure 
# (i.e., when declaring segment:offset as two separate 16-bit fields, place the offset first 
# and then follow with the segment because x86 is little-endian).

# (2) If the disk drive itself does not support LBA addressing, 
# the BIOS will automatically convert the LBA to a CHS address for you -- so this function still works.

# (3) The transfer buffer should be 16-bit (2 byte) aligned.

dap: # disk access packet
    .byte 0x10 # size of dap (16 bytes)
    .byte 0    # always 0
dap_sector_num:
    .word 0    # number of sectors
dap_buf_offset:
    .word 0    # offset to memory buffer
dap_buf_seg:
    .word 0    # segment of memory buffer
dap_start_lba:
    .quad 0    # start logical block address

# Magic number 0x55 0xaa
.org 510
.word 0xaa55
