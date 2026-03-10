fn main() {
    // This project uses sqlx offline mode
    // To prepare queries, run: cargo sqlx prepare --database-url sqlite:dhcp.db
    println!("cargo:rerun-if-changed=migrations");
}
