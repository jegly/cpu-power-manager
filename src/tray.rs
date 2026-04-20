/// System tray via StatusNotifierItem (zbus, no C libdbus headers needed).
/// Gracefully no-ops if the DBus session bus is unavailable (e.g. sudo without -E).
use gtk4::prelude::*;
use gtk4::ApplicationWindow;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
enum TrayMsg { Toggle, Quit }

pub fn spawn(window: ApplicationWindow) {
    let queue: Arc<Mutex<Vec<TrayMsg>>> = Arc::new(Mutex::new(Vec::new()));
    let queue_tray = queue.clone();
    let queue_gtk  = queue.clone();

    // Poll the message queue every 250ms on the GTK main thread.
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let msgs: Vec<TrayMsg> = std::mem::take(&mut queue_gtk.lock().unwrap());
        for msg in msgs {
            match msg {
                TrayMsg::Toggle => {
                    if window.is_visible() { window.hide(); } else { window.present(); }
                }
                TrayMsg::Quit => window.close(),
            }
        }
        glib::ControlFlow::Continue
    });

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => { log::warn!("Tray: runtime error: {}", e); return; }
        };
        rt.block_on(run_tray(queue_tray));
    });
}

async fn run_tray(queue: Arc<Mutex<Vec<TrayMsg>>>) {
    use zbus::interface;

    struct Notifier { queue: Arc<Mutex<Vec<TrayMsg>>> }

    #[interface(name = "org.kde.StatusNotifierItem")]
    impl Notifier {
        #[zbus(property)] fn id(&self) -> &str { "cpu-power-manager" }
        #[zbus(property)] fn title(&self) -> &str { "CPU Power Manager" }
        #[zbus(property)] fn status(&self) -> &str { "Active" }
        #[zbus(property)] fn icon_name(&self) -> &str { "cpu" }
        #[zbus(property)] fn category(&self) -> &str { "SystemServices" }
        #[zbus(property)] fn item_is_menu(&self) -> bool { false }

        fn activate(&self, _x: i32, _y: i32) {
            self.queue.lock().unwrap().push(TrayMsg::Toggle);
        }
        fn context_menu(&self, _x: i32, _y: i32) {}
        fn scroll(&self, _delta: i32, _orientation: &str) {}
    }

    let builder = match zbus::connection::Builder::session() {
        Ok(b) => b,
        Err(e) => { log::warn!("Tray: no session bus: {}", e); return; }
    };
    let builder = match builder.name("org.kde.StatusNotifierItem-cpu-power-manager") {
        Ok(b) => b,
        Err(e) => { log::warn!("Tray: failed to claim name: {}", e); return; }
    };
    let builder = match builder.serve_at("/StatusNotifierItem", Notifier { queue: queue.clone() }) {
        Ok(b) => b,
        Err(e) => { log::warn!("Tray: serve_at failed: {}", e); return; }
    };
    let conn = match builder.build().await {
        Ok(c) => c,
        Err(e) => { log::warn!("Tray: connection failed: {}", e); return; }
    };

    if let Ok(proxy) = zbus::Proxy::new(
        &conn,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    ).await {
        let name = conn.unique_name().map(|n| n.to_string()).unwrap_or_default();
        let _ = proxy.call_method("RegisterStatusNotifierItem",
            &(format!("{}/StatusNotifierItem", name).as_str(),)).await;
        log::info!("System tray registered");
    }

    std::future::pending::<()>().await;
}
