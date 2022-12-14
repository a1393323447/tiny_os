use std::{process::Command, path::Path, io::{Read, Write}};

fn build_kernel() {
    println!("[Build]: Building libkernel.a ...");

    let mut cargo = Command::new(env!("CARGO"));

    // 构建 kernel elf
    println!("[Build]: Building kernel elf ...");
    cargo.current_dir("./kernel");
    cargo.arg("build")
        .arg("--profile").arg("kernel")
        .arg("--package").arg("kernel")
        .arg("--target").arg("./x86_64-os.json")
        .arg("-Z").arg("unstable-options")
        .arg("-Zbuild-std=core,compiler_builtins,alloc")
        .arg("-Zbuild-std-features=compiler-builtins-mem");
    let output = cargo.output().expect("[Error]: Failed to run cargo to build kernel elf");
    if !output.status.success() {
        panic!("[Error]: Failed to build kernel elf: \n{}", String::from_utf8_lossy(&output.stderr));
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));

    // 使用 llvm 工具将构建好的 kernel elf 文件进行修改:
    // 1. 使用 objcopy 去除 debug 信息, 重定义符号, 最后将 kernel elf 文件包装在一个新的 elf 文件中
    // 2. 使用 ar 将第一步得到的 elf 文件包装成一个静态链接库, 在构建 bootloader 时, 
    //    将 bootloader 和 kernel 链接在一起
    //
    // 1. 先获取 objcopy 和 ar 的本地路径
    let llvm_tools = llvm_tools::LlvmTools::new().expect("[Error]: Can not setup llvm tools");
    let objcopy_path = llvm_tools.tool(&llvm_tools::exe("llvm-objcopy"))
        .expect("[Error]: Can not found llvm-objcopy. Please install llvm-objcopy.");
    let ar_path = llvm_tools.tool(&llvm_tools::exe("llvm-ar"))
        .expect("[Error]: Can not found llvm-ar. Please install llvm-ar.");
    
    // 2. 使用 objcopy 去除 debug 信息    
    println!("[Build]: Try to strip debug info in kernel elf.");
    let mut objcopy = Command::new(&objcopy_path);
    objcopy.current_dir("./target/x86_64-os/kernel");
    objcopy.arg("--strip-debug")
           .arg("kernel")
           .arg("kernel_strip");
    let output = objcopy.output().expect("[Error]: Failed to run llvm-objcopy to strip debug info in kernel.");
    if !output.status.success() {
        panic!("[Error]: Failed to strip debug info in kernel: \n{}", String::from_utf8_lossy(&output.stderr));
    }

    // 3. 使用 objcopy 重定义一系列符号, 然后将 kernel 的 elf 文件当作一个 bin 文件包装在一个新的 elf 文件中
    println!("[Build]: Try to wrap kernel elf in a new elf file.");
    let mut objcopy = Command::new(&objcopy_path);
    objcopy.current_dir("./target/x86_64-os/kernel");
    objcopy.arg("-I").arg("binary")
           .arg("-O").arg("elf64-x86-64")
           .arg("--binary-architecture=i386:x86-64")
           .arg("--rename-section").arg(".data=.kernel")
           .arg("--redefine-sym").arg("_binary_kernel_strip_start=_kernel_start_addr")    
           .arg("--redefine-sym").arg("_binary_kernel_strip_end=_kernel_end_addr")
           .arg("--redefine-sym").arg("_binary_kernel_strip_size=_kernel_size")
           .arg("kernel_strip")
           .arg("kernel_redef");
    let output = objcopy.output()
        .expect("[Error]: Failed to run objcopy to redefine syms and warp kernel elf in a new elf file.");
    if !output.status.success() {
        panic!("[Error]: Failed to redefine syms and warp kernel elf in a new elf file: {}", 
                String::from_utf8_lossy(&output.stderr)
        );
    }

    // 4. 使用 ar 将上一步得到的 elf 文件包装成一个静态链接库
    println!("[Build]: Try to wrap new kernel elf file in a static lib ...");
    let mut ar = Command::new(&ar_path);
    ar.arg("crs")
      .arg("./target/libkernel.a")
      .arg("./target/x86_64-os/kernel/kernel_redef");
    let output = ar.output()
        .expect("[Error]: Failed to run ar to warp kernel as a static lib.");
    if !output.status.success() {
        panic!("[Error]: Failed to warp kernel as a static lib: {}", 
                String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("[Build]: Finished: ./target/libkernel.a");
}

fn build_bootloader() {
    println!("[Build]: Building bootloader elf ...");
    let mut cargo = Command::new(env!("CARGO"));
    // 构建 bootloader
    cargo.current_dir("./boot");
    cargo.arg("build")
        .arg("--profile").arg("bootloader")
        .arg("--package").arg("boot")
        .arg("--target").arg("./x86_64-bootloader.json")
        .arg("-Z").arg("unstable-options")
        .arg("-Zbuild-std=core")
        .arg("-Zbuild-std-features=compiler-builtins-mem")
        .arg("--quiet");

    let output = cargo.output().expect("[Error]: Failed to run bootloader build script");
    if !output.status.success() {
        panic!("[Error]: Failed to build bootloader: \n{}", String::from_utf8_lossy(&output.stderr));
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));

    println!("[Build]: Finished ./target/x86_64-bootloader/bootloader/boot");
}

fn create_disk_image(
    bootloader_elf_path: &Path,
    output_bin_path: &Path
) {
    println!("[Build]: Building os.img ...");

    let llvm_tools = llvm_tools::LlvmTools::new().expect("Can not setup llvm tools");
    let objcopy_path = llvm_tools.tool(&llvm_tools::exe("llvm-objcopy"))
        .expect("[Error]: Can not found llvm-objcopy. Please install llvm-objcopy or rust-objcopy.");

    // 将 elf 格式的文件变成 bin 格式
    let dir = bootloader_elf_path.parent().unwrap();
    let temp_bin = dir.join("temp_bin");
    let mut objcopy = Command::new(objcopy_path);
    objcopy.arg("-I").arg("elf64-x86-64")
        .arg("-O").arg("binary")
        .arg("--binary-architecture=i386:x86-64")
        .arg(bootloader_elf_path)
        .arg(&temp_bin);
    let output = objcopy.output().expect("[Error]: Failed to run objcopy to create disk image");
    if !output.status.success() {
        panic!("[Error]: Failed to create disk image: \n{}", String::from_utf8_lossy(&output.stderr));
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));
    
    let mut file = std::fs::File::open(&temp_bin).expect("[Error]: Failed to open temp bin file");
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).expect("[Error]: Failed to read temp bin file");

    drop(file);

    const BLOCK_SIZE: usize = 512;
    let mut file = std::fs::File::create(output_bin_path).expect("[Error]: Failed to create bin file");

    for pos in 0..bytes.len()-1 {
        if bytes[pos] == 0x55 && bytes[pos + 1] == 0xaa {
            let bin = &bytes[pos - (BLOCK_SIZE - 2)..];
            file.write_all(bin).expect("[Error]: Failed to copy bin");
            // pad to 512 align
            let reminder = bin.len() % BLOCK_SIZE;
            if reminder != 0 {
                let pad = vec![0u8; BLOCK_SIZE - reminder];
                file.write_all(&pad).expect("[Error]: Failed to pad bin");
                println!("[Build]: image size = {} bytes", bin.len() + pad.len());
            } else {
                println!("[Build]: image size = {} bytes", bin.len());
            }

            println!("[Build]: Build Finished. os.img: ./target/os.img");
            return;
        }
    }

    unreachable!("[Error]: can not find magic number 0xaa55 in temp bin file");
}

fn main() {
    // 构建 kernel
    build_kernel();
    // 构建 bootloader
    build_bootloader();
    // 构建 bin 文件
    let bootloader_elf_path = Path::new("target/x86_64-bootloader/bootloader/boot");
    let output_bin_path = Path::new("./target/os.img");
    create_disk_image(bootloader_elf_path, output_bin_path);
}