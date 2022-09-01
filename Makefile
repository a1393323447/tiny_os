clean:
	cargo clean

build-release:
	cargo run --release

build-debug:
	cargo run --release # todo

run-qemu: clean build-release
	dd if=target/os.img of=bochs/os.img bs=512 count=50000 conv=notrunc
	qemu-system-x86_64 -drive format=raw,file=target/os.img -boot c

run-bochs: clean build-release
	dd if=target/os.img of=bochs/os.img bs=512 count=50000 conv=notrunc
	bochs -q -f bochs/conf/bochsrc.bxrc

debug-qemu: clean build-debug
	qemu-system-x86_64 -drive format=raw,file=target/os.img -boot c -s -S &
	gdb target/x86_64-bootloader/bootloader/boot --eval-command="target remote :1234"

debug-bochs: clean build-debug
	dd if=target/os.img of=bochs/os.img bs=512 count=50000 conv=notrunc
	bochsdbg -q -f bochs/conf/bochsdbg-gdb.bxrc &
	gdb target/x86_64-bootloader/bootloader/boot --eval-command="target remote :1234"
