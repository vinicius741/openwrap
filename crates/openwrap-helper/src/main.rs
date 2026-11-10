#[cfg(unix)]
use openwrap_helper::connect::run_connect;
#[cfg(unix)]
use openwrap_helper::reconcile::run_reconcile_dns;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let exit_code = match std::env::args().nth(1).as_deref() {
        Some("connect") => run_connect().await,
        Some("reconcile-dns") => run_reconcile_dns(),
        _ => {
            eprintln!("usage: openwrap-helper connect|reconcile-dns");
            64
        }
    };
    std::process::exit(exit_code);
}

#[cfg(not(unix))]
fn main() {
    eprintln!("openwrap-helper is only supported on Unix platforms");
    std::process::exit(64);
}
