# builder
`builder` 主要用于制作 `Tiny OS` 的镜像文件, 主要步骤为:
- 将 `kernel` 编译生成 `elf` 文件 —— `kernel`
- 去除 `kernel` 文件的 `debug` 信息得到文件 `kernel_strip`
- 将 `kernel_strip` 包装在一个新的 `elf` 文件中, 并重命名一些段的名称, 得到文件 `kernel_redef`
- 将 `kernel_redef` 打包成一个静态链接库 `libkernel.a`
- 将 `boot` 编译并与 `libkernel.a` 进行链接生成 `elf` 文件 `bootloader`
- 通过 `objcpoy` 将 `bootloader` 转换成 `binary` 文件 `bootloader.bin`
- 最后将 `bootloader.bin` 的大小填充到扇区大小的整数倍, 生成镜像文件 `os.img`