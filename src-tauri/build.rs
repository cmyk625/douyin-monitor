fn main() {
    #[cfg(windows)]
    {
        // 说明：Tauri 默认会把「Common Controls v6 依赖」作为清单资源嵌入可执行文件，
        // 而 cargo test 生成的测试二进制没有清单，导入 TaskDialogIndirect 时会在加载阶段
        // 以 STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) 退出。
        // 这里改为：不让 tauri-build 嵌入清单资源，统一由 .cargo/config.toml 的
        // /MANIFESTDEPENDENCY 链接参数生成清单（内容与 Tauri 默认清单完全一致），
        // 这样应用二进制与测试二进制行为统一，也不会出现资源重复 (CVT1100)。
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        tauri_build::try_build(attributes).expect("tauri-build 执行失败");
    }
    #[cfg(not(windows))]
    {
        tauri_build::build();
    }
}
