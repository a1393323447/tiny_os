# 这个文件记录的是通过 0x13 中断读取硬盘的代码, 
# 经过多次重构, 在 bootloader 中被淘汰

# https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
# 有些机器不支持扩展 bios 中断, 所以要先检查机器是否支持
check_int_0x13:
    mov ah, 0x41
    mov dl, 0x80
    mov bx, 0x55aa
    int 0x13
    jc  port_read_hdd

bios_read_hdd:
    call bios_read
    jmp to_stage_2


# To read a disk:
#   Set the proper values in the disk address packet structure
#   Set DS:SI -> Disk Address Packet in memory
#   Set AH = 0x42
#   Set DL = "drive number" -- typically 0x80 for the "C" drive
#   Issue an INT 0x13.
# The carry flag will be set if there is any error during the transfer. AH should be set to 0 on success.
# To write to a disk, set AH = 0x43.
# 理论上我们最多一次读 127 个 sector -> 127 * 512 = 65024 字节
# 我们的 bootloader 不会大于 65204 字节
# 所以只读一次即可
bios_read:
    # init dap
    # sector num
    mov eax, offset _rest_of_bootloader_start_addr
    mov ebx, offset _rest_of_bootloader_end_addr
    sub ebx, eax
    shr ebx, 9  # div 512
                # linker.ld 中 _rest_of_bootloader_start_addr 紧接着 .boot-first-stage ,
                # _rest_of_bootloader_end_addr 以 512 字节对齐
                # 而 .boot-first-stage 正好 512 字节
                # 故它们的差也一定是 512 字节的倍数, 一定整除, 不用向上取整
    mov [dap_sector_num], bx
    # 多次测试可以发现下面的参数都比较固定
    # 所以直接填在 DAP 中了
    # seg and offset
    # mov eax, offset _rest_of_bootloader_start_addr # 定义在 linker.ld
    # 物理地址 = 段基地址 * 16 + 偏移地址
    # dap buffer addr segment
    # 段基地址要除以 16
    # mov ebx, eax
    # shr ebx, 4    # div by 16, ebx = 段基地址
    # mov [dap_buf_seg], bx
    # buffer offset
    # 偏移地址 = 物理地址 - 段基地址 * 16
    # shl ebx, 4
    # sub eax, ebx
    # mov [dap_buf_offset], ax
    # LBA addr
    # 计算剩余部分的起始扇区号 
    # mov eax, offset _rest_of_bootloader_start_addr                   
    # mov ebx, offset _start
    # sub eax, ebx
    # shr eax, 9   # div 512 -> 一定整除 -> 理论上这一定是 1
    # mov [dap_start_lba], eax
    # set regs
    mov si, offset dap
    mov ah, 0x42
    mov dl, 0x80 # 从主盘读取
    # read
    int 0x13
    # check
    jc port_read_hdd    # error, try port read hdd

    ret


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
dap:
    .byte  0x10  # 16 bytes
    .byte   0    # always 0
dap_sector_num:
    .word   2
dap_buf_offset:
    .word 0x7e00
dap_buf_seg:
    .word   0
dap_start_lba:
    .quad   1
