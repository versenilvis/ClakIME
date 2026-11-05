use std::process::Command;

#[test]
fn test_doctor_cli_no_privacy_leak() {
    let cargo_bin = env!("CARGO_BIN_EXE_clak");

    // test markdown mode
    let md_out = Command::new(cargo_bin)
        .args(["doctor", "--markdown"])
        .output()
        .expect("failed to run clak doctor --markdown");
    assert!(md_out.status.success());
    let md_str = String::from_utf8_lossy(&md_out.stdout);

    // test standard terminal output
    let cli_out = Command::new(cargo_bin)
        .arg("doctor")
        .output()
        .expect("failed to run clak doctor");
    assert!(cli_out.status.success());
    let cli_str = String::from_utf8_lossy(&cli_out.stdout);

    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            assert!(
                !md_str.contains(&home),
                "clak doctor --markdown leaked $HOME: {}",
                home
            );
            assert!(
                !cli_str.contains(&home),
                "clak doctor leaked $HOME: {}",
                home
            );
        }
    }

    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() && user.len() >= 3 && user != "arch" && user != "linux" {
            let home_user = format!("/home/{}", user);
            assert!(
                !md_str.contains(&home_user),
                "clak doctor --markdown leaked /home/$USER"
            );
            assert!(
                !cli_str.contains(&home_user),
                "clak doctor leaked /home/$USER"
            );
        }
    }

    // verify GTK_IM_MODULE and /dev/uinput explanation
    assert!(
        md_str.contains("/dev/uinput"),
        "markdown must contain /dev/uinput"
    );
    assert!(
        md_str.contains("GTK_IM_MODULE"),
        "markdown must contain GTK_IM_MODULE"
    );
    assert!(
        md_str.contains("tầng xử lý"),
        "markdown must mention processing tier (tầng xử lý)"
    );
    assert!(
        cli_str.contains("tầng"),
        "cli output must mention processing tier (tầng)"
    );
}
