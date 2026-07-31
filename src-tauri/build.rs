fn main() {
    tauri_build::build();

    // MinGW 交叉编译时 tauri-build 不会把 icon.ico 嵌进 .exe，
    // 任务栏会显示兜底的 Tauri 红色大象。这里显式补上。
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        // workspace 视角，build.rs 在 src-tauri/ 下执行
        res.set_icon("icons/icon.ico");
        res.compile().expect("failed to compile Windows resources (icon.ico)");
    }
}
