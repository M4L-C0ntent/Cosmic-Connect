// src/cosmic-connect-sms.rs
//! Binary entry point for the SMS window application.

fn main() -> cosmic::iced::Result {
    // Setup signal handlers for graceful shutdown
    setup_signal_handlers();
    
    let args: Vec<String> = std::env::args().collect();
    
    let device_id = args.get(1).cloned().unwrap_or_else(|| "unknown".to_string());
    let device_name = args.get(2).cloned().unwrap_or_else(|| "Unknown Device".to_string());
    
    eprintln!("=== KDE Connect SMS Window ===");
    eprintln!("Device: {} ({})", device_name, device_id);
    
    let result = cosmic_connect_applet::plugins::sms::run(device_id, device_name);
    
    // Ensure cleanup happens
    eprintln!("SMS window closing, cleaning up...");
    cleanup_on_exit();
    
    result
}

fn setup_signal_handlers() {
    use std::sync::atomic::{AtomicBool, Ordering};
    
    static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
    
    ctrlc::set_handler(move || {
        if SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst) {
            eprintln!("Force shutdown");
            std::process::exit(1);
        }
        
        eprintln!("Graceful shutdown requested...");
        cleanup_on_exit();
        std::process::exit(0);
    })
    .ok(); // Ignore error if already set
}

fn cleanup_on_exit() {
    // Create a minimal tokio runtime for cleanup
    let rt = tokio::runtime::Runtime::new();
    if let Ok(rt) = rt {
        rt.block_on(async {
            cosmic_connect_applet::dbus::cleanup().await;
        });
    }
}