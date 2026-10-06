use std::env;
use std::fs;
use std::path::PathBuf;

pub fn enable_autostart() -> Result<(), String> {
    let home = env::var("HOME").map_err(|e| e.to_string())?;
    let home_path = PathBuf::from(home);

    let autostart_dir = home_path.join(".config/autostart");
    fs::create_dir_all(&autostart_dir).map_err(|e| e.to_string())?;
    let desktop_file = autostart_dir.join("clak-autostart.desktop");

    let desktop_content = "[Desktop Entry]\n\
Type=Application\n\
Name=Clak Vietnamese Input Method\n\
Comment=Autostart Fcitx5 with Clak input method\n\
Exec=fcitx5 -d\n\
Icon=org.fcitx.Fcitx5\n\
Terminal=false\n\
Categories=System;Utility;\n\
StartupNotify=false\n\
X-GNOME-Autostart-Phase=Applications\n\
X-GNOME-AutoRestart=true\n\
X-GNOME-Autostart-Notify=false\n\
X-KDE-autostart-after=panel\n";

    fs::write(&desktop_file, desktop_content).map_err(|e| e.to_string())?;

    let env_dir = home_path.join(".config/environment.d");
    fs::create_dir_all(&env_dir).map_err(|e| e.to_string())?;
    let env_file = env_dir.join("99-clak-im.conf");

    let env_content = "GTK_IM_MODULE=fcitx\n\
QT_IM_MODULE=fcitx\n\
XMODIFIERS=@im=fcitx\n\
INPUT_METHOD=fcitx5\n\
SDL_IM_MODULE=fcitx\n";

    fs::write(&env_file, env_content).map_err(|e| e.to_string())?;

    let profile_file = home_path.join(".config/fcitx5/profile");
    if profile_file.exists() {
        if let Ok(mut prof) = fs::read_to_string(&profile_file) {
            if !prof.contains("Name=clak") {
                let count = prof.matches("[Groups/0/Items/").count();
                let clak_entry = format!("\n[Groups/0/Items/{}]\nName=clak\n", count);
                if let Some(idx) = prof.find("[GroupOrder]") {
                    prof.insert_str(idx, &clak_entry);
                } else {
                    prof.push_str(&clak_entry);
                }
            }

            let lines: Vec<String> = prof
                .lines()
                .map(|line| {
                    if line.starts_with("DefaultIM=") {
                        "DefaultIM=clak".to_string()
                    } else {
                        line.to_string()
                    }
                })
                .collect();

            let new_prof = lines.join("\n") + "\n";
            let _ = fs::write(&profile_file, new_prof);
        }
    }

    Ok(())
}

pub fn disable_autostart() -> Result<(), String> {
    let home = env::var("HOME").map_err(|e| e.to_string())?;
    let home_path = PathBuf::from(home);

    let desktop_file = home_path.join(".config/autostart/clak-autostart.desktop");
    if desktop_file.exists() {
        let _ = fs::remove_file(desktop_file);
    }

    let env_file = home_path.join(".config/environment.d/99-clak-im.conf");
    if env_file.exists() {
        let _ = fs::remove_file(env_file);
    }

    Ok(())
}
