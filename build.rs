use std::io;

fn main() -> io::Result<()> {
    // 设置 CMake 策略版本，解决旧 CMakeLists.txt 与新 CMake 的兼容问题
    println!("cargo:rustc-env=CMAKE_POLICY_VERSION_MINIMUM=3.5");

    #[cfg(windows)]
    {
        winres::WindowsResource::new()
            .set_icon("assets/swyh-rs-2.ico")
            .compile()?;
    }
    Ok(())
}