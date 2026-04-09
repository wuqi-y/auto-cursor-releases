use tauri::{
    Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

// 引入日志宏 - 这些宏通过 #[macro_export] 导出到 crate 根部
use crate::{log_info, log_warn};

// 创建一个简单的托盘图标（16x16 像素，RGBA 格式）
fn create_simple_rgba_icon() -> (Vec<u8>, u32, u32) {
    let width = 16u32;
    let height = 16u32;
    let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
    
    // 创建一个简单的蓝色圆形图标
    for y in 0..height {
        for x in 0..width {
            let center_x = width as f32 / 2.0;
            let center_y = height as f32 / 2.0;
            let distance = ((x as f32 - center_x).powi(2) + (y as f32 - center_y).powi(2)).sqrt();
            
            if distance <= 6.0 {
                // 蓝色圆形
                rgba_data.extend_from_slice(&[0x00, 0x7A, 0xFF, 0xFF]); // 蓝色，完全不透明
            } else {
                // 透明背景
                rgba_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 完全透明
            }
        }
    }
    
    (rgba_data, width, height)
}

// 创建备用图标
fn create_fallback_icon() -> tauri::image::Image<'static> {
    let (rgba_data, width, height) = create_simple_rgba_icon();
    tauri::image::Image::new_owned(rgba_data, width, height)
}

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    log_info!("🔧 [TRAY] 开始创建系统托盘...");

    // 创建托盘菜单项
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_item,
        &hide_item,
        &separator,
        &quit_item,
    ])?;

    log_info!("🔧 [TRAY] 托盘菜单创建成功，包含 {} 个项目", 4);

    // 使用应用默认图标或创建简单图标
    let tray_icon = match app.default_window_icon() {
        Some(icon) => {
            log_info!("🔧 [TRAY] 使用应用默认图标");
            icon.clone()
        }
        None => {
            log_warn!("⚠️ [TRAY] 未找到默认图标，创建简单图标");
            create_fallback_icon()
        }
    };

    // 创建系统托盘 - 使用简化的方式
    let _tray = TrayIconBuilder::new()
        .icon(tray_icon)
        .tooltip("Auto Cursor - 点击显示/隐藏窗口")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        log_info!("🔍 [TRAY] 从托盘菜单显示窗口");
                    }
                }
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                        log_info!("🫥 [TRAY] 从托盘菜单隐藏窗口");
                    }
                }
                "quit" => {
                    log_info!("🚪 [TRAY] 用户从托盘菜单退出应用");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                            log_info!("🫥 [TRAY] 左键单击托盘图标：窗口已隐藏");
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                            log_info!("🔍 [TRAY] 左键单击托盘图标：窗口已显示");
                        }
                    }
                }
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    log_info!("🖱️ [TRAY] 右键单击托盘图标：显示菜单");
                }
                _ => {
                    log_info!("🖱️ [TRAY] 其他托盘事件: {:?}", event);
                }
            }
        })
        .build(app)?;

    log_info!("✅ [TRAY] 系统托盘创建成功");
    Ok(())
}
