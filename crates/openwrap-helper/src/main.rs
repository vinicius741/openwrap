mod connect;
mod reconcile;
mod request;
mod system;
mod tests;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let exit_code = match std::env::args().nth(1).as_deref() {
        Some("connect") => connect::run_connect().await,
        Some("reconcile-dns") => reconcile::run_reconcile_dns(),
        _ => {
            eprintln!("usage: openwrap-helper connect|reconcile-dns");
            64
        }
    };
    std::process::exit(exit_code);
}
