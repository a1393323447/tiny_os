fn main() {
    // 链接 kernel
    println!("cargo:rustc-link-search=native=./target");
    println!("cargo:rustc-link-lib=static=kernel");
}